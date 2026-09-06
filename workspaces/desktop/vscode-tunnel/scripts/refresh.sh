APP_NAME="$1"

if [ "${APP_NAME,,}" == "cursor" ]; then
  cursor tunnel restart
else
  code tunnel restart
fi
