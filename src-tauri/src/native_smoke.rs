//! Explicit fixture build only. No command, path access or capability is added.
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
pub fn illustrated_enabled() -> bool {
    std::env::var("DIARY_ILLUSTRATED_GUI_SMOKE").is_ok_and(|v| v == "1") || upload_enabled()
}
fn upload_enabled() -> bool {
    std::env::var("DIARY_ILLUSTRATED_UPLOAD_GUI_SMOKE").is_ok_and(|v| v == "1")
}
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if illustrated_enabled() {
        app.set_activation_policy(tauri::ActivationPolicy::Regular);
        app.handle().show()?;
        let main = app
            .get_webview_window("main")
            .ok_or("missing main window")?;
        main.show()?;
        main.set_focus()?;
        tauri::async_runtime::spawn(async move {
            for _ in 0..180 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let close = main.clone();
                let _ = main.eval_with_callback(
                    "JSON.stringify(window.__illustratedSmoke || null)",
                    move |result| {
                        eprintln!("SMOKE illustrated_gui {result}");
                        if result.contains("passed") || result.contains("failed") {
                            let _ = close.close();
                        }
                    },
                );
            }
            let _ = main.close();
        });
        return Ok(());
    }
    WebviewWindowBuilder::new(app,"denied-smoke",WebviewUrl::App("index.html".into()))
        .title("Scaffold permission test").visible(false)
        .initialization_script(r#"window.addEventListener('DOMContentLoaded', async()=>{
          try {await window.__TAURI_INTERNALS__.invoke('get_app_metadata');window.location.href='tauri://localhost/smoke-unexpected-allow';}
          catch(_){window.location.href='tauri://localhost/smoke-denied';}
        });"#)
        .on_navigation(|url|{
            match url.path(){
                "/smoke-denied"=>{eprintln!("SMOKE secondary_command_denied");false},
                "/smoke-unexpected-allow"=>{eprintln!("SMOKE FAIL secondary_command_allowed");false},
                _=>true,
            }
        }).build()?;
    let main = app
        .get_webview_window("main")
        .ok_or("missing main window")?;
    app.set_activation_policy(tauri::ActivationPolicy::Regular);
    app.handle().show()?;
    main.show()?;
    main.set_focus()?;
    tauri::Webview::set_focus(main.as_ref())?;
    tauri::async_runtime::spawn(async move {
        for _ in 0..7 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            eprintln!(
                "SMOKE window_probe visible={} focused={}",
                main.is_visible().unwrap_or(false),
                main.is_focused().unwrap_or(false)
            );
            let _=main.eval_with_callback("JSON.stringify({focused:document.hasFocus(),activeIsEditor:document.activeElement?.classList.contains('cm-content'),editable:document.querySelector('.cm-content')?.getAttribute('contenteditable'),step:window.__smokeStep||'none',createDisabled:document.querySelector('button[aria-label=\"Create Empty\"]')?.disabled,error:window.__smokeError||null,notice:[...document.querySelectorAll('.notice')].map(n=>n.textContent).join(' / '),length:document.querySelector('.cm-content')?.textContent.length,keys:window.__smokeKeys||0,trusted:window.__smokeTrusted||0,matched:document.querySelector('.cm-content')?.textContent.includes('synthetic keyboard sentinel')})",|result|eprintln!("SMOKE editor_probe {result}"));
        }
        let _ = main.close();
    });
    Ok(())
}

pub fn main_loaded(webview: &tauri::Webview) -> Result<(), Box<dyn std::error::Error>> {
    webview.set_focus()?;
    if illustrated_enabled() {
        webview.eval(if upload_enabled() {
            include_str!("native_smoke/illustrated-upload.js")
        } else {
            include_str!("native_smoke/illustrated.js")
        })?;
        return Ok(());
    }
    webview.eval(
        r#"(async()=>{
      try {
        const invoke=window.__TAURI_INTERNALS__.invoke;
        await invoke('get_app_metadata');
        // Automatic connection opens the date before Create Empty becomes available.
        // Scratch buffers alone can be editable even before the UI has connected.
        window.__smokeStep='ui-ready';
        let editor;
        for(let attempt=0;attempt<100;attempt++){
          const create=document.querySelector('button[aria-label="Create Empty"]');
          editor=document.querySelector('.editor-surface .cm-content');
          if(create&&!create.disabled&&editor?.getAttribute('contenteditable')==='true')break;
          editor=null;
          await new Promise(resolve=>setTimeout(resolve,50));
        }
        if(!editor)throw Error('automatic UI connection/editor readiness timeout');
        if(window.location.origin!=='tauri://localhost')throw Error('unexpected packaged origin');
        await new Promise((resolve,reject)=>{
          const timer=setTimeout(()=>reject(Error('missing CSP violation')),1500);
          const listener=event=>{if(event.violatedDirective==='connect-src'&&event.blockedURI.startsWith('http://127.0.0.1:1')){clearTimeout(timer);document.removeEventListener('securitypolicyviolation',listener);resolve();}};
          document.addEventListener('securitypolicyviolation',listener);
          fetch('http://127.0.0.1:1/api/v1/status',{mode:'no-cors'}).catch(()=>{});
        });
        window.__smokeStep='settings';const settings=await invoke('get_settings');
        if(!settings.folder_checks.some(check=>check.alias==='smoke-readable'&&check.outcome==='readable'))throw Error('server readable folder check');
        if(!settings.folder_checks.some(check=>check.alias==='smoke-denied'&&check.outcome==='denied'))throw Error('server denied folder check');
        const jobs=await invoke('list_jobs',{cursor:null});
        if(!jobs.jobs.some(job=>job.id==='scaffold-fake-job'&&job.state==='running'))throw Error('missing fake job');
        let completed,failed;
        const observed=new Promise((resolve,reject)=>{completed=resolve;failed=reject;});
        let delivery=0;
        const id=window.__TAURI_INTERNALS__.transformCallback(raw=>{
          if('end' in raw)return;
          const packet=raw.message;
          if(packet.delivery_id!==++delivery){failed(Error('delivery ordering'));return;}
          invoke('subscribe_events',{request:{action:'acknowledge',subscription_id:packet.subscription_id,delivery_id:packet.delivery_id}}).then(()=>{
            if(packet.event.kind==='job_changed'&&packet.event.payload.state==='cancelled')completed();
            if(packet.event.kind==='disconnected')failed(Error('stream disconnected'));
          }).catch(failed);
        },false);
        window.__smokeStep='sub';await invoke('subscribe_events',{request:{action:'start',channel:'__CHANNEL__:'+id}});window.__smokeStep='subbed';
        window.__smokeKeys=0;window.__smokeTrusted=0;
        document.addEventListener('keydown',event=>{window.__smokeKeys++;if(event.isTrusted)window.__smokeTrusted++;});
        editor.focus();
        if(document.activeElement!==editor)throw Error('native focus failed');
        await invoke('get_job',{id:'scaffold-fake-job'});
        let typed=false;
        for(let attempt=0;attempt<180;attempt++){
          if(editor.textContent.includes('synthetic keyboard sentinel')){typed=true;break;}
          // Loading the date resets the editor state, which can drop focus; keep it focused until the keys land.
          if(document.activeElement!==editor)editor.focus();
          await new Promise(resolve=>setTimeout(resolve,50));
        }
        if(!typed)throw Error('native keyboard input missing');
        await invoke('cancel_job',{id:'scaffold-fake-job'});
        await Promise.race([observed,new Promise((_,reject)=>setTimeout(()=>reject(Error('channel timeout')),3000))]);
        const cancelled=await invoke('get_job',{id:'scaffold-fake-job'});
        if(cancelled.state!=='cancelled')throw Error('cancel not authoritative');
        await invoke('close_session');
        await invoke('connect_server');
        await invoke('open_session');
        const reconnected=await invoke('get_job',{id:'scaffold-fake-job'});
        if(reconnected.state!=='running')throw Error('fresh reconnect failed');
      } catch (e) { window.__smokeError=String(e&&e.message||e); console.error('Native smoke main failed'); }
    })();"#,
    )?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FolderCheck {
    pub alias: String,
    pub outcome: String,
}
