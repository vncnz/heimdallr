#!/usr/bin/env bash

ID=$(notify-send \
  --print-id \
  --app-name="nemo" \
  --urgency=critical \
  --icon="media-removable" \
  --hint=string:desktop-entry:org.Nemo \
  "Unmounting External256" \
  "Disconnecting from filesystem.")

sleep 0.1

notify-send \
  --replace-id="$ID" \
  --app-name="nemo" \
  --urgency=normal \
  --icon="media-removable" \
  --hint=string:desktop-entry:org.Nemo \
  "External256 unmounted" \
  "Filesystem has been disconnected."



# [09:56:44.373] app_name:nemo summary:Unmounting External256 body:Disconnecting from filesystem. timeout:-1 hints:{"desktop-entry": Str(Str(Borrowed("org.Nemo"))), "image-path": Str(Str(Borrowed("media-removable"))), "urgency": U8(2)} replaces_id:0

# [09:56:45.839] app_name:nemo summary:External256 unmounted body:Filesystem has been disconnected. timeout:-1 hints:{"desktop-entry": Str(Str(Borrowed("org.Nemo"))), "urgency": U8(1), "image-path": Str(Str(Borrowed("media-removable")))} replaces_id:6