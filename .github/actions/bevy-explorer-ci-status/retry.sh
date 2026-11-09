#!/usr/bin/env bash

backoff_delay() {
  local attempt="$1" maximum="$2" delay="${3:-1}" step=1
  if [ "$delay" -gt "$maximum" ]; then
    delay="$maximum"
  fi
  while [ "$step" -lt "$attempt" ]; do
    if [ "$delay" -ge "$maximum" ] || [ "$delay" -gt "$((maximum / 2))" ]; then
      delay="$maximum"
      break
    else
      delay=$((delay * 2))
    fi
    step=$((step + 1))
  done
  printf '%s\n' "$delay"
}

sleep_for() {
  local delay="$1" remaining
  if [ "$#" -gt 1 ]; then
    remaining="$2"
    [ "$remaining" -gt 0 ] || return 124
    if [ "$remaining" -lt "$delay" ]; then
      delay="$remaining"
    fi
  fi
  sleep "$delay" || return "$?"
}

retry_after_attempt() {
  local attempt="$1" attempts="$2" maximum="$3" delay
  [ "$attempt" -lt "$attempts" ] || return 0
  delay=$(backoff_delay "$attempt" "$maximum" "${4:-}") || return "$?"
  sleep_for "$delay"
}
