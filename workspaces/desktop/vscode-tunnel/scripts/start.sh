APP_NAME="$1"

if [ "${APP_NAME,,}" == "cursor" ]; then
  node /vscode-tunnel/scripts/cursor-start.js
else
  node /vscode-tunnel/scripts/init-or-start.js $APP_NAME
fi

