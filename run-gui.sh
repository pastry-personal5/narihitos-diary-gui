#!/usr/bin/env bash
export DISCOVERY=`find ./runtime_root -name "discvoery.json"`
npm run tauri dev -- -- -- --connect "$(DISCOVERY)"
