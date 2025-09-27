#!/usr/bin/bash

# If the log file exists, it will display new log entries in real-time.
LOGFILE_TTS="/tmp/moshi_tts.log"
LOGFILE_STT="/tmp/moshi_stt.log"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

if [ -f "$LOGFILE_TTS" ]; then
    echo -e "${GREEN}==== TTS LOG (${LOGFILE_TTS}) ====${NC}"
    tail -f "$LOGFILE_TTS" &
else
    echo "Log file does not exist: $LOGFILE_TTS"
fi

if [ -f "$LOGFILE_STT" ]; then
    echo -e "${RED}==== STT LOG (${LOGFILE_STT}) ====${NC}"
    tail -f "$LOGFILE_STT" &
else
    echo "Log file does not exist: $LOGFILE_STT"
fi

wait