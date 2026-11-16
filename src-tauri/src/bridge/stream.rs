//! The server-sent event stream: subscription, acknowledgement and shutdown.
use super::*;

impl Bridge {
    pub fn acknowledge(&self, id: &str, delivery: u64) -> Result<(), &'static str> {
        match self.flow.lock().expect("flow poisoned").as_ref() {
            Some(flow) => flow.acknowledge(id, delivery),
            None => Ok(()),
        }
    }
    pub async fn detach_events(&self) {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_events().await;
    }
    pub async fn stop_events(&self) {
        if let Some(flow) = self.flow.lock().expect("flow poisoned").take() {
            flow.stop();
        }
        if let Some(task) = self.subscription.lock().await.take() {
            task.abort();
        }
    }
    pub async fn subscribe(
        &self,
        channel: tauri::ipc::Channel<Delivery>,
    ) -> Result<(), &'static str> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_events().await;
        let flow = Flow::new();
        *self.flow.lock().expect("flow poisoned") = Some(flow.clone());
        let c = self.connection.lock().await;
        let d = c.discovery.as_ref().ok_or("disconnected")?;
        let session = c.session.clone().ok_or("session_unavailable")?;
        let instance = d.instance_id.clone();
        let cursor = c.cursor.clone();
        // Establish a complete bounded paginated baseline before resuming hints.
        // Use the first page's watermark: events racing later pages are replayed.
        let mut page_cursor: Option<String> = None;
        let mut watermark = None;
        let mut pages = 0;
        let mut seen = std::collections::HashSet::new();
        loop {
            let path = page_cursor
                .as_ref()
                .map_or_else(|| "/jobs".into(), |id| format!("/jobs?cursor={id}"));
            let page: JobList = self.request(&c, Method::GET, &path, None).await?;
            page.validate(page_cursor.as_deref(), Some((&instance, &session)))?;
            if page.jobs.iter().any(|job| !seen.insert(job.id.clone())) {
                return Err("invalid_snapshot");
            }
            let first = watermark.is_none();
            watermark.get_or_insert(page.event_cursor);
            page_cursor = page.next_cursor;
            flow.send(
                &channel,
                BridgeEvent::JobSnapshot {
                    instance_id: instance.clone(),
                    session_id: session.clone(),
                    jobs: page.jobs,
                    first,
                    complete: false,
                },
            )
            .await?;
            pages += 1;
            if page_cursor.is_none() {
                break;
            }
            if pages >= 1024 {
                return Err("snapshot_limit");
            }
        }
        let last = watermark.unwrap_or(0);
        *cursor.lock().expect("cursor poisoned") = Cursor::at(last);
        // Streaming has an idle deadline, never the ordinary 5-second total deadline.
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(5))
            .read_timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|_| "transport_unavailable")?;
        let response = client
            .get(format!("{}/api/v1/events", d.origin))
            .bearer_auth(&d.token)
            .header("x-diary-session", &session)
            .header("x-diary-trace-id", new_trace())
            .header("x-diary-instance", &instance)
            .header("last-event-id", last.to_string())
            .header("x-diary-reconcile", "1")
            .send()
            .await
            .map_err(|_| "server_unavailable")?;
        if !response.status().is_success()
            || response
                .headers()
                .get("x-diary-reconcile")
                .and_then(|v| v.to_str().ok())
                != Some("1")
            || response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .is_none_or(|v| v.split(';').next() != Some("text/event-stream"))
        {
            return Err("invalid_event_stream");
        }
        let snapshot_client = self.client.clone();
        let origin = d.origin.clone();
        let token = d.token.clone();
        let task = tauri::async_runtime::spawn(async move {
            let mut ready = false;
            let mut dirty = std::collections::HashSet::<String>::new();
            let mut response = response;
            let mut decoder = Decoder::default();
            let outcome: Result<(), &'static str> = async {
                while let Some(chunk) = response.chunk().await.map_err(|_| "stream_disconnected")? {
                    for frame in decoder.feed(&chunk)? {
                        let event = cursor
                            .lock()
                            .expect("cursor poisoned")
                            .decode(&frame, &instance, &session)?;
                        if let Some(event) = event {
                            if matches!(event, BridgeEvent::ReplayComplete) {
                                if ready {
                                    return Err("invalid_event");
                                }
                                for id in dirty.drain() {
                                    let job = progress::reconcile_job(
                                        &snapshot_client,
                                        &origin,
                                        &token,
                                        &instance,
                                        &session,
                                        &id,
                                    )
                                    .await?;
                                    flow.send(
                                        &channel,
                                        BridgeEvent::JobSnapshot {
                                            instance_id: instance.clone(),
                                            session_id: session.clone(),
                                            jobs: vec![job],
                                            first: false,
                                            complete: false,
                                        },
                                    )
                                    .await?;
                                }
                                flow.send(
                                    &channel,
                                    BridgeEvent::JobSnapshot {
                                        instance_id: instance.clone(),
                                        session_id: session.clone(),
                                        jobs: vec![],
                                        first: false,
                                        complete: true,
                                    },
                                )
                                .await?;
                                ready = true;
                                continue;
                            }
                            if !ready
                                && let BridgeEvent::JobChanged { job_id, .. }
                                | BridgeEvent::CandidateReady { job_id, .. } = &event
                            {
                                if dirty.len() >= 4096 && !dirty.contains(job_id) {
                                    return Err("snapshot_limit");
                                }
                                dirty.insert(job_id.clone());
                            }
                            let resync = matches!(event, BridgeEvent::ResyncRequired { .. });
                            flow.send(&channel, event).await?;
                            if resync {
                                return Ok(());
                            }
                        }
                    }
                }
                Err("stream_disconnected")
            }
            .await;
            if outcome.is_err() {
                flow.disconnected(&channel);
            }
        });
        *self.subscription.lock().await = Some(task);
        Ok(())
    }
}
