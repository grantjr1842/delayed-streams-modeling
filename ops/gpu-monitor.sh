#!/usr/bin/env bash

set -euo pipefail

INTERVAL_SECONDS="${1:-1}"
COMPACT="${COMPACT:-1}"
SORT_PROCS="${SORT_PROCS:-1}"
SHOW_PROCS="${SHOW_PROCS:-1}"
SHOW_BARS="${SHOW_BARS:-1}"
MAX_PROC_ROWS="${MAX_PROC_ROWS:-20}"
VRAM_BAR_WIDTH="${VRAM_BAR_WIDTH:-18}"
UTIL_BAR_WIDTH="${UTIL_BAR_WIDTH:-10}"

TICK_SECONDS=0.1

if ! [[ "$INTERVAL_SECONDS" =~ ^[0-9]+$ ]]; then
  echo "Interval must be an integer number of seconds." >&2
  exit 1
fi

if ! command -v nvidia-smi >/dev/null 2>&1; then
  echo "nvidia-smi not found. Install NVIDIA drivers or run this on a GPU host." >&2
  exit 1
fi

if ! nvidia-smi --query-gpu=name --format=csv,noheader >/dev/null 2>&1; then
  echo "nvidia-smi is present but not responding. Is the NVIDIA driver running?" >&2
  exit 1
fi

TERM_HAS_TPUT=0
COLOR_ENABLED=1
if [[ -n "${NO_COLOR:-}" ]]; then
  COLOR_ENABLED=0
fi
if [[ "${TERM:-}" == "dumb" ]]; then
  COLOR_ENABLED=0
fi
if ! [[ -t 1 ]]; then
  COLOR_ENABLED=0
fi
if command -v tput >/dev/null 2>&1 && [[ -t 1 ]]; then
  TERM_HAS_TPUT=1
fi

if [[ "$TERM_HAS_TPUT" -eq 1 && "$COLOR_ENABLED" -eq 1 ]]; then
  COLOR_BOLD="$(tput bold 2>/dev/null || true)"
  COLOR_RESET="$(tput sgr0 2>/dev/null || true)"
  COLOR_DIM="$(tput dim 2>/dev/null || true)"
  COLOR_CYAN="$(tput setaf 6 2>/dev/null || true)"
  COLOR_GREEN="$(tput setaf 2 2>/dev/null || true)"
  COLOR_BLUE="$(tput setaf 4 2>/dev/null || true)"
  COLOR_RED="$(tput setaf 1 2>/dev/null || true)"
  COLOR_MAGENTA="$(tput setaf 5 2>/dev/null || true)"
  COLOR_WHITE="$(tput setaf 7 2>/dev/null || true)"
  COLOR_REVERSE="$(tput rev 2>/dev/null || true)"
else
  COLOR_BOLD=""
  COLOR_RESET=""
  COLOR_DIM=""
  COLOR_CYAN=""
  COLOR_GREEN=""
  COLOR_BLUE=""
  COLOR_RED=""
  COLOR_MAGENTA=""
  COLOR_WHITE=""
  COLOR_REVERSE=""
fi

cleanup() {
  if [[ "$TERM_HAS_TPUT" -eq 1 ]]; then
    tput rmcup 2>/dev/null || true
    tput cnorm 2>/dev/null || true
  fi
}

if [[ "$TERM_HAS_TPUT" -eq 1 ]]; then
  tput smcup 2>/dev/null || true
  tput civis 2>/dev/null || true
  trap cleanup EXIT INT TERM
fi

INPUT_FD=0
if [[ -r /dev/tty ]]; then
  exec 3</dev/tty
  INPUT_FD=3
fi

trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf "%s" "$s"
}

safe_int() {
  local v="$1"
  if [[ "$v" =~ ^[0-9]+$ ]]; then
    echo "$v"
  else
    echo 0
  fi
}

normalize_bool() {
  local v="${1:-}"
  v="${v,,}"
  case "$v" in
    1|true|yes|on)
      echo 1
      ;;
    0|false|no|off|"")
      echo 0
      ;;
    *)
      echo 0
      ;;
  esac
}

normalize_nonneg_int() {
  local v
  v="$(safe_int "${1:-}")"
  echo "$v"
}

COMPACT="$(normalize_bool "$COMPACT")"
SORT_PROCS="$(normalize_bool "$SORT_PROCS")"
SHOW_PROCS="$(normalize_bool "$SHOW_PROCS")"
SHOW_BARS="$(normalize_bool "$SHOW_BARS")"
MAX_PROC_ROWS="$(normalize_nonneg_int "$MAX_PROC_ROWS")"
VRAM_BAR_WIDTH="$(normalize_nonneg_int "$VRAM_BAR_WIDTH")"
UTIL_BAR_WIDTH="$(normalize_nonneg_int "$UTIL_BAR_WIDTH")"

format_mb() {
  local mib="$1"
  echo $((mib * 1048576 / 1000000))
}

format_gb() {
  local mib="$1"
  local tenths=$((mib * 1048576 * 10 / 1000000000))
  local int=$((tenths / 10))
  local frac=$((tenths % 10))
  printf "%d.%d" "$int" "$frac"
}

visible_length() {
  local text="$1"
  local len=${#text}
  local i=0
  local count=0
  while (( i < len )); do
    local ch="${text:i:1}"
    if [[ "$ch" == $'\x1b' && "${text:i+1:1}" == "[" ]]; then
      i=$((i + 2))
      while (( i < len )); do
        local c="${text:i:1}"
        i=$((i + 1))
        if [[ "$c" =~ [[:alpha:]] ]]; then
          break
        fi
      done
      continue
    fi
    count=$((count + 1))
    i=$((i + 1))
  done
  printf "%s" "$count"
}

fit_text() {
  local text="$1"
  local width="$2"
  if (( width <= 0 )); then
    printf "%s" ""
    return
  fi

  if [[ "$text" != *$'\x1b['* ]]; then
    if (( ${#text} <= width )); then
      printf "%-*s" "$width" "$text"
    else
      if (( width >= 3 )); then
        printf "%-*s" "$width" "${text:0:width-3}..."
      else
        printf "%-*s" "$width" "${text:0:width}"
      fi
    fi
    return
  fi

  local visible_len
  visible_len="$(visible_length "$text")"
  if (( visible_len <= width )); then
    printf "%s" "$text"
    local pad=$((width - visible_len))
    if (( pad > 0 )); then
      printf "%*s" "$pad" ""
    fi
    return
  fi

  local target_width="$width"
  local ellipsis=""
  if (( width >= 3 )); then
    target_width=$((width - 3))
    ellipsis="..."
  fi

  local out=""
  local count=0
  local i=0
  local len=${#text}
  local saw_ansi=0
  while (( i < len && count < target_width )); do
    local ch="${text:i:1}"
    if [[ "$ch" == $'\x1b' && "${text:i+1:1}" == "[" ]]; then
      saw_ansi=1
      local seq="$ch"
      i=$((i + 1))
      while (( i < len )); do
        local c="${text:i:1}"
        seq+="$c"
        i=$((i + 1))
        if [[ "$c" =~ [[:alpha:]] ]]; then
          break
        fi
      done
      out+="$seq"
      continue
    fi
    out+="$ch"
    count=$((count + 1))
    i=$((i + 1))
  done
  out+="$ellipsis"
  if (( saw_ansi )); then
    out+="${COLOR_RESET}"
  fi
  printf "%s" "$out"
}

make_bar() {
  local used="$1"
  local total="$2"
  local width="$3"
  local color="$4"
  if (( width <= 0 )) || [[ "$SHOW_BARS" -eq 0 ]]; then
    printf "%s" ""
    return
  fi

  local filled=0
  if (( total > 0 )); then
    filled=$(( used * width / total ))
    if (( filled > width )); then
      filled=$width
    fi
  fi

  local empty=$((width - filled))
  local fill=""
  local emp=""
  local tip=""

  if (( filled > 0 )); then
    if (( filled >= 2 )); then
      fill="$(printf "%$((filled - 1))s" "" | tr ' ' '=')"
      tip=">"
    else
      tip=">"
    fi
  fi

  if (( empty > 0 )); then
    emp="$(printf "%${empty}s" "" | tr ' ' '.')"
  fi

  printf "%s%s%s%s%s%s" "${color}${COLOR_BOLD}" "$fill$tip" "${COLOR_RESET}" "${COLOR_DIM}${COLOR_WHITE}" "$emp" "${COLOR_RESET}"
}

print_at() {
  local row="$1"
  local col="$2"
  local text="${3-}"
  if [[ "$TERM_HAS_TPUT" -eq 1 ]]; then
    tput cup "$row" "$col" 2>/dev/null || true
    printf "%s" "$text"
    tput el 2>/dev/null || true
  else
    printf "\033[%d;%dH%s\033[K" "$((row + 1))" "$((col + 1))" "$text"
  fi
}

move_cursor() {
  local row="$1"
  local col="$2"
  if [[ "$TERM_HAS_TPUT" -eq 1 ]]; then
    tput cup "$row" "$col" 2>/dev/null || true
  else
    printf "\033[%d;%dH" "$((row + 1))" "$((col + 1))"
  fi
}

clear_screen() {
  if [[ "$TERM_HAS_TPUT" -eq 1 ]]; then
    tput cup 0 0 2>/dev/null || true
    tput ed 2>/dev/null || true
  else
    printf "\033[2J\033[H"
  fi
}

refresh_term_size() {
  local cols=""
  local rows=""

  if [[ "$TERM_HAS_TPUT" -eq 1 ]]; then
    cols="$(tput cols 2>/dev/null || true)"
    rows="$(tput lines 2>/dev/null || true)"
  fi

  if [[ -z "$cols" || -z "$rows" ]]; then
    if [[ -n "${COLUMNS:-}" && -n "${LINES:-}" ]]; then
      cols="$COLUMNS"
      rows="$LINES"
    fi
  fi

  if [[ -z "$cols" || -z "$rows" ]]; then
    if command -v stty >/dev/null 2>&1; then
      local stty_out
      stty_out="$(stty size </dev/tty 2>/dev/null || true)"
      if [[ -n "$stty_out" ]]; then
        rows="${stty_out%% *}"
        cols="${stty_out##* }"
      fi
    fi
  fi

  if ! [[ "$cols" =~ ^[0-9]+$ ]]; then
    cols=120
  fi
  if ! [[ "$rows" =~ ^[0-9]+$ ]]; then
    rows=40
  fi

  TERM_COLS="$cols"
  TERM_ROWS="$rows"
}

TERM_COLS=120
TERM_ROWS=40
DETAIL_LINES_PER_GPU=3
GPU_COUNT=0
PROC_ROWS_SHOWN=0
DETAILS_VISIBLE=1
PROC_COUNT=0
SELECTED_PROC_INDEX=0
CURSOR_PID=""
PROC_DELIM=$'\x1f'

GPU_W_GPU=3
GPU_W_USED=18
GPU_W_TOTAL=18
GPU_W_UTIL=4
GPU_W_TEMP=4
GPU_W_VRAM=24
GPU_W_VRAMBAR=0
GPU_W_UTILBAR=0

PROC_W_SEL=3
PROC_W_PID=7
PROC_W_GPU=3
PROC_W_USED=18
PROC_W_BAR=0
PROC_W_CMD=20

LINE_TITLE=0
LINE_TIME=0
LINE_STATUS=0
LINE_PAUSE=0
GPU_HEADER_ROW=0
GPU_ROW_START=0
PROC_SECTION_ROW=0
PROC_HEADER_ROW=0
PROC_ROW_START=0
DETAIL_SECTION_ROW=0
DETAIL_ROW_START=0
CONTROLS_ROW=0
CONTROLS_TEXT_ROW=0
PROMPT_ROW=0

NEEDS_REDRAW=1

trap 'NEEDS_REDRAW=1' WINCH

GPU_INDEXES=()
GPU_NAMES=()
GPU_UUIDS=()
GPU_DRIVERS=()
GPU_TOTALS=()
declare -A GPU_TOTAL_BY_INDEX

declare -A GPU_INDEX_BY_UUID

declare -A GPU_ROW_BY_INDEX
PROC_ENTRIES=()
PROC_PIDS=()
declare -A SELECTED_PIDS=()
declare -A PROC_USED_MB_BY_PID
declare -A PROC_USED_KNOWN_BY_PID
declare -A PROC_USED_RAW_BY_PID
declare -A PROC_GPU_BY_PID
declare -A PROC_CMD_BY_PID
declare -A PROC_NAME_BY_PID
PROC_VRAM_AVAILABLE=0
PROC_VRAM_UNKNOWN=0
PROC_SHOW_VRAM=1

load_gpu_info() {
  GPU_COUNT=0
  GPU_INDEXES=()
  GPU_NAMES=()
  GPU_UUIDS=()
  GPU_DRIVERS=()
  GPU_TOTALS=()
  GPU_TOTAL_BY_INDEX=()
  GPU_INDEX_BY_UUID=()

  while IFS=',' read -r idx name uuid driver mem_total; do
    idx="$(trim "$idx")"
    name="$(trim "$name")"
    uuid="$(trim "$uuid")"
    driver="$(trim "$driver")"
    mem_total="$(safe_int "$(trim "$mem_total")")"

    GPU_INDEXES+=("$idx")
    GPU_NAMES+=("$name")
    GPU_UUIDS+=("$uuid")
    GPU_DRIVERS+=("$driver")
    GPU_TOTALS+=("$mem_total")
    GPU_TOTAL_BY_INDEX["$idx"]="$mem_total"
    GPU_INDEX_BY_UUID["$uuid"]="$idx"
    GPU_COUNT=$((GPU_COUNT + 1))
  done < <(nvidia-smi --query-gpu=index,name,uuid,driver_version,memory.total --format=csv,noheader,nounits)
}

calc_column_widths() {
  if [[ "$COMPACT" -eq 1 ]]; then
    DETAIL_LINES_PER_GPU=1
    GPU_W_GPU=3
    GPU_W_VRAM=24
    GPU_W_UTIL=4
    GPU_W_USED=0
    GPU_W_TOTAL=0
    GPU_W_TEMP=0
  else
    DETAIL_LINES_PER_GPU=3
    GPU_W_GPU=3
    GPU_W_USED=18
    GPU_W_TOTAL=18
    GPU_W_UTIL=4
    GPU_W_TEMP=4
  fi

  local base_cols=0
  if [[ "$COMPACT" -eq 1 ]]; then
    base_cols=$((GPU_W_GPU + 1 + GPU_W_VRAM + 1 + GPU_W_UTIL))
  else
    base_cols=$((GPU_W_GPU + 1 + GPU_W_USED + 1 + GPU_W_TOTAL + 1 + GPU_W_UTIL + 1 + GPU_W_TEMP))
  fi

  local available=$((TERM_COLS - base_cols - 2))
  if (( available < 6 )) || [[ "$SHOW_BARS" -eq 0 ]]; then
    GPU_W_VRAMBAR=0
    GPU_W_UTILBAR=0
  else
    local desired=$((VRAM_BAR_WIDTH + UTIL_BAR_WIDTH + 1))
    GPU_W_VRAMBAR="$VRAM_BAR_WIDTH"
    GPU_W_UTILBAR="$UTIL_BAR_WIDTH"
    if (( desired > available )); then
      GPU_W_VRAMBAR=$((available * VRAM_BAR_WIDTH / desired))
      GPU_W_UTILBAR=$((available - GPU_W_VRAMBAR - 1))
      if (( GPU_W_UTILBAR < 4 )); then
        GPU_W_UTILBAR=0
        GPU_W_VRAMBAR=$available
      fi
    fi
  fi

  PROC_W_SEL=3
  PROC_W_PID=7
  PROC_W_GPU=3
  PROC_W_USED=18
  PROC_W_BAR=$GPU_W_VRAMBAR

  local proc_base=$((PROC_W_SEL + 1 + PROC_W_PID + 1 + PROC_W_GPU + 1 + PROC_W_USED + 1))
  if (( PROC_W_BAR > 0 )); then
    proc_base=$((proc_base + PROC_W_BAR + 1))
  fi
  PROC_W_CMD=$((TERM_COLS - proc_base))
  if (( PROC_W_CMD < 10 )); then
    PROC_W_BAR=0
    proc_base=$((PROC_W_SEL + 1 + PROC_W_PID + 1 + PROC_W_GPU + 1 + PROC_W_USED + 1))
    PROC_W_CMD=$((TERM_COLS - proc_base))
    if (( PROC_W_CMD < 5 )); then
      PROC_W_CMD=5
    fi
  fi
}

calc_layout() {
  refresh_term_size

  calc_column_widths

  DETAILS_VISIBLE=1
  PROC_ROWS_SHOWN=$MAX_PROC_ROWS

  while true; do
    local detail_rows=0
    if [[ "$DETAILS_VISIBLE" -eq 1 ]]; then
      detail_rows=$((GPU_COUNT * DETAIL_LINES_PER_GPU))
    fi

    local fixed_rows=$((4 + 1 + 1 + GPU_COUNT + 1 + 1 + 1 + 1 + 1 + detail_rows + 1 + 1 + 1 + 1))
    local available=$((TERM_ROWS - fixed_rows))
    if (( available < 0 )); then
      if [[ "$DETAILS_VISIBLE" -eq 1 ]]; then
        DETAILS_VISIBLE=0
        continue
      fi
      available=0
    fi

    if (( available < MAX_PROC_ROWS )); then
      PROC_ROWS_SHOWN=$available
    else
      PROC_ROWS_SHOWN=$MAX_PROC_ROWS
    fi
    break
  done

  local row=0
  LINE_TITLE=$row
  row=$((row + 1))
  LINE_TIME=$row
  row=$((row + 1))
  LINE_STATUS=$row
  row=$((row + 1))
  LINE_PAUSE=$row
  row=$((row + 1))

  GPU_HEADER_ROW=$row
  row=$((row + 1))
  GPU_ROW_START=$row
  row=$((row + GPU_COUNT))
  row=$((row + 1))

  PROC_SECTION_ROW=$row
  row=$((row + 1))
  PROC_HEADER_ROW=$row
  row=$((row + 1))
  PROC_ROW_START=$row
  row=$((row + PROC_ROWS_SHOWN))
  row=$((row + 1))

  DETAIL_SECTION_ROW=$row
  row=$((row + 1))
  DETAIL_ROW_START=$row
  if [[ "$DETAILS_VISIBLE" -eq 1 ]]; then
    row=$((row + GPU_COUNT * DETAIL_LINES_PER_GPU))
  fi
  row=$((row + 1))

  CONTROLS_ROW=$row
  row=$((row + 1))
  CONTROLS_TEXT_ROW=$row
  row=$((row + 1))
  PROMPT_ROW=$row

  GPU_ROW_BY_INDEX=()
  for i in "${!GPU_INDEXES[@]}"; do
    GPU_ROW_BY_INDEX["${GPU_INDEXES[$i]}"]=$((GPU_ROW_START + i))
  done
}

render_static() {
  clear_screen

  print_at "$LINE_TITLE" 0 "${COLOR_BOLD}${COLOR_CYAN}== GPU monitor ==${COLOR_RESET}"
  print_at "$LINE_TIME" 0 "${COLOR_BLUE}Time:${COLOR_RESET} --"
  print_at "$LINE_STATUS" 0 "${COLOR_BLUE}Status:${COLOR_RESET} --"
  print_at "$LINE_PAUSE" 0 ""

  local gpu_header=""
  if [[ "$COMPACT" -eq 1 ]]; then
    gpu_header="$(printf "${COLOR_BOLD}%-*s %-*s %-*s" "$GPU_W_GPU" "GPU" "$GPU_W_VRAM" "VRAM Alloc(MB)/Avail(GB)" "$GPU_W_UTIL" "GPU Busy%")"
  else
    gpu_header="$(printf "${COLOR_BOLD}%-*s %-*s %-*s %-*s %-*s" "$GPU_W_GPU" "GPU" "$GPU_W_USED" "VRAM Alloc(MB/GB)" "$GPU_W_TOTAL" "VRAM Avail(MB/GB)" "$GPU_W_UTIL" "GPU Busy%" "$GPU_W_TEMP" "TEMP")"
  fi
  if (( GPU_W_VRAMBAR > 0 )); then
    gpu_header+=" $(fit_text "VRAM Alloc" "$GPU_W_VRAMBAR")"
  fi
  if (( GPU_W_UTILBAR > 0 )); then
    gpu_header+=" $(fit_text "GPU Busy%" "$GPU_W_UTILBAR")"
  fi
  gpu_header+="${COLOR_RESET}"

  print_at "$GPU_HEADER_ROW" 0 "$(fit_text "$gpu_header" "$TERM_COLS")"
  for ((i = 0; i < GPU_COUNT; i++)); do
    print_at "$((GPU_ROW_START + i))" 0 ""
  done

  print_at "$PROC_SECTION_ROW" 0 "${COLOR_BOLD}${COLOR_CYAN}== GPU processes ==${COLOR_RESET}"
  local proc_header="$(printf "${COLOR_BOLD}%-*s %-*s %-*s %-*s" "$PROC_W_SEL" "SEL" "$PROC_W_PID" "PID" "$PROC_W_GPU" "GPU" "$PROC_W_USED" "VRAM Alloc(MB/GB)")"
  if (( PROC_W_BAR > 0 )); then
    proc_header+=" $(fit_text "VRAM Alloc" "$PROC_W_BAR")"
  fi
  proc_header+=" $(fit_text "COMMAND" "$PROC_W_CMD")${COLOR_RESET}"
  print_at "$PROC_HEADER_ROW" 0 "$(fit_text "$proc_header" "$TERM_COLS")"
  for ((i = 0; i < PROC_ROWS_SHOWN; i++)); do
    print_at "$((PROC_ROW_START + i))" 0 ""
  done

  if [[ "$DETAILS_VISIBLE" -eq 1 ]]; then
    print_at "$DETAIL_SECTION_ROW" 0 "${COLOR_BOLD}${COLOR_CYAN}== GPU details ==${COLOR_RESET}"
    for i in "${!GPU_INDEXES[@]}"; do
      local base_row=$((DETAIL_ROW_START + i * DETAIL_LINES_PER_GPU))
      if [[ "$COMPACT" -eq 1 ]]; then
        local total_mb
        local total_gb
        total_mb="$(format_mb "${GPU_TOTALS[$i]}")"
        total_gb="$(format_gb "${GPU_TOTALS[$i]}")"
        local line="  ${COLOR_BOLD}GPU ${GPU_INDEXES[$i]}:${COLOR_RESET} ${GPU_NAMES[$i]} | ${COLOR_BLUE}Drv${COLOR_RESET} ${GPU_DRIVERS[$i]} | ${COLOR_GREEN}VRAM Total${COLOR_RESET} ${total_mb}MB/${total_gb}GB"
        print_at "$base_row" 0 "$(fit_text "$line" "$TERM_COLS")"
      else
        local total_mb
        local total_gb
        total_mb="$(format_mb "${GPU_TOTALS[$i]}")"
        total_gb="$(format_gb "${GPU_TOTALS[$i]}")"
        local line1="  ${COLOR_BOLD}GPU ${GPU_INDEXES[$i]}:${COLOR_RESET} ${GPU_NAMES[$i]} ${COLOR_MAGENTA}(UUID: ${GPU_UUIDS[$i]})${COLOR_RESET}"
        local line2="    ${COLOR_BLUE}Driver:${COLOR_RESET} ${GPU_DRIVERS[$i]}"
        local line3="    ${COLOR_GREEN}VRAM Total:${COLOR_RESET} ${total_mb} MB (${total_gb} GB)"
        print_at "$base_row" 0 "$(fit_text "$line1" "$TERM_COLS")"
        print_at "$((base_row + 1))" 0 "$(fit_text "$line2" "$TERM_COLS")"
        print_at "$((base_row + 2))" 0 "$(fit_text "$line3" "$TERM_COLS")"
      fi
    done
  else
    print_at "$DETAIL_SECTION_ROW" 0 "${COLOR_BOLD}${COLOR_CYAN}== GPU details (hidden; resize) ==${COLOR_RESET}"
  fi

  print_at "$CONTROLS_ROW" 0 "${COLOR_BOLD}${COLOR_CYAN}== Controls ==${COLOR_RESET}"
  print_at "$CONTROLS_TEXT_ROW" 0 "$(fit_text "${COLOR_BOLD}[up/down]${COLOR_RESET} move, ${COLOR_BOLD}[space]${COLOR_RESET} select, ${COLOR_BOLD}[k]${COLOR_RESET} kill, ${COLOR_BOLD}[p]${COLOR_RESET} pause, ${COLOR_BOLD}[t]${COLOR_RESET} procs, ${COLOR_BOLD}[s]${COLOR_RESET} sort, ${COLOR_BOLD}[c]${COLOR_RESET} compact, ${COLOR_BOLD}[b]${COLOR_RESET} bars, ${COLOR_BOLD}[q]${COLOR_RESET} quit" "$TERM_COLS")"
  print_at "$PROMPT_ROW" 0 ""
}

update_status() {
  local paused="$1"
  local show_procs="$2"
  local status="running"
  if [[ "$paused" -eq 1 ]]; then
    status="paused"
  fi
  local proc_state="off"
  if [[ "$show_procs" -eq 1 ]]; then
    proc_state="on"
  fi
  local sort_state="off"
  if [[ "$SORT_PROCS" -eq 1 ]]; then
    if [[ "$PROC_SHOW_VRAM" -eq 0 ]]; then
      sort_state="pid"
    else
      sort_state="vram alloc"
    fi
  fi
  local compact_state="off"
  if [[ "$COMPACT" -eq 1 ]]; then
    compact_state="on"
  fi
  local bars_state="off"
  if [[ "$SHOW_BARS" -eq 1 ]]; then
    bars_state="on"
  fi

  local selected_count="${#SELECTED_PIDS[@]}"
  local vram_state=""
  if (( PROC_COUNT > 0 )); then
    if [[ "$PROC_VRAM_AVAILABLE" -eq 0 && "$PROC_VRAM_UNKNOWN" -eq 1 ]]; then
      vram_state=" | ${COLOR_BLUE}Proc VRAM:${COLOR_RESET} N/A"
    elif [[ "$PROC_VRAM_AVAILABLE" -eq 1 && "$PROC_VRAM_UNKNOWN" -eq 1 ]]; then
      vram_state=" | ${COLOR_BLUE}Proc VRAM:${COLOR_RESET} partial"
    fi
  fi
  print_at "$LINE_STATUS" 0 "${COLOR_BLUE}Status:${COLOR_RESET} ${status} | ${COLOR_BLUE}Procs:${COLOR_RESET} ${proc_state} | ${COLOR_BLUE}Sort:${COLOR_RESET} ${sort_state} | ${COLOR_BLUE}Compact:${COLOR_RESET} ${compact_state} | ${COLOR_BLUE}Bars:${COLOR_RESET} ${bars_state} | ${COLOR_BLUE}Selected:${COLOR_RESET} ${selected_count}${vram_state}"
  if [[ "$paused" -eq 1 ]]; then
    print_at "$LINE_PAUSE" 0 "${COLOR_RED}PAUSED${COLOR_RESET} - press 'p' to resume"
  else
    print_at "$LINE_PAUSE" 0 ""
  fi
}

update_gpu_rows() {
  while IFS=',' read -r idx used total util temp; do
    idx="$(trim "$idx")"
    used="$(safe_int "$(trim "$used")")"
    total="$(safe_int "$(trim "$total")")"
    util="$(safe_int "$(trim "$util")")"
    temp="$(safe_int "$(trim "$temp")")"
    local avail=$((total - used))
    if (( avail < 0 )); then
      avail=0
    fi

    local row="${GPU_ROW_BY_INDEX[$idx]:-$GPU_ROW_START}"
    local vram_bar="$(make_bar "$used" "$total" "$GPU_W_VRAMBAR" "$COLOR_GREEN")"
    local util_bar="$(make_bar "$util" 100 "$GPU_W_UTILBAR" "$COLOR_MAGENTA")"
    local used_mb
    local total_gb
    used_mb="$(format_mb "$used")"
    total_gb="$(format_gb "$total")"
    local used_gb
    local avail_mb
    local avail_gb
    used_gb="$(format_gb "$used")"
    avail_mb="$(format_mb "$avail")"
    avail_gb="$(format_gb "$avail")"

    local line=""
    if [[ "$COMPACT" -eq 1 ]]; then
      local vram_pair="${used_mb}MB/${avail_gb}GB"
      line="$(printf "%-*s ${COLOR_GREEN}%-*s${COLOR_RESET} ${COLOR_MAGENTA}%-*s${COLOR_RESET}" "$GPU_W_GPU" "$idx" "$GPU_W_VRAM" "$vram_pair" "$GPU_W_UTIL" "${util}%")"
    else
      local alloc_label="${used_mb}MB/${used_gb}GB"
      local avail_label="${avail_mb}MB/${avail_gb}GB"
      line="$(printf "%-*s ${COLOR_GREEN}%-*s${COLOR_RESET} %-*s ${COLOR_MAGENTA}%-*s${COLOR_RESET} ${COLOR_RED}%-*s${COLOR_RESET}" "$GPU_W_GPU" "$idx" "$GPU_W_USED" "$alloc_label" "$GPU_W_TOTAL" "$avail_label" "$GPU_W_UTIL" "${util}%" "$GPU_W_TEMP" "$temp")"
    fi

    if (( GPU_W_VRAMBAR > 0 )); then
      line+=" $vram_bar"
    fi
    if (( GPU_W_UTILBAR > 0 )); then
      line+=" $util_bar"
    fi

    print_at "$row" 0 "$(fit_text "$line" "$TERM_COLS")"
  done < <(nvidia-smi --query-gpu=index,memory.used,memory.total,utilization.gpu,temperature.gpu --format=csv,noheader,nounits)
}

reconcile_selection() {
  if (( PROC_COUNT == 0 )); then
    SELECTED_PROC_INDEX=0
    CURSOR_PID=""
    return
  fi

  local found=-1
  if [[ -n "$CURSOR_PID" ]]; then
    for i in "${!PROC_PIDS[@]}"; do
      if [[ "${PROC_PIDS[$i]}" == "$CURSOR_PID" ]]; then
        found=$i
        break
      fi
    done
  fi

  if (( found >= 0 )); then
    SELECTED_PROC_INDEX=$found
  else
    if (( SELECTED_PROC_INDEX >= PROC_COUNT )); then
      SELECTED_PROC_INDEX=$((PROC_COUNT - 1))
    fi
    if (( SELECTED_PROC_INDEX < 0 )); then
      SELECTED_PROC_INDEX=0
    fi
    CURSOR_PID="${PROC_PIDS[$SELECTED_PROC_INDEX]}"
  fi
}

prune_selected_pids() {
  local -A active=()
  for pid in "${PROC_PIDS[@]}"; do
    active["$pid"]=1
  done
  for pid in "${!SELECTED_PIDS[@]}"; do
    if [[ -z "${active[$pid]+x}" ]]; then
      unset 'SELECTED_PIDS[$pid]'
    fi
  done
}

load_process_entries() {
  PROC_ENTRIES=()
  PROC_PIDS=()
  PROC_USED_MB_BY_PID=()
  PROC_USED_KNOWN_BY_PID=()
  PROC_USED_RAW_BY_PID=()
  PROC_GPU_BY_PID=()
  PROC_CMD_BY_PID=()
  PROC_NAME_BY_PID=()
  PROC_COUNT=0
  PROC_VRAM_AVAILABLE=0
  PROC_VRAM_UNKNOWN=0
  PROC_SHOW_VRAM=1

  if [[ "$SHOW_PROCS" -eq 0 ]]; then
    return
  fi

  local proc_lines
  proc_lines="$(nvidia-smi --query-compute-apps=pid,gpu_uuid,used_memory,process_name --format=csv,noheader,nounits 2>/dev/null || true)"
  if [[ -z "$proc_lines" ]]; then
    reconcile_selection
    prune_selected_pids
    return
  fi

  local -a proc_raw=()
  local -a pids=()
  while IFS=',' read -r pid gpu_uuid used_raw proc_name; do
    pid="$(trim "$pid")"
    gpu_uuid="$(trim "$gpu_uuid")"
    used_raw="$(trim "$used_raw")"
    proc_name="$(trim "$proc_name")"
    [[ -z "$pid" ]] && continue
    local used_mb=0
    local used_known=0
    if [[ "$used_raw" =~ ^[0-9]+$ ]]; then
      used_mb="$used_raw"
      used_known=1
    fi
    proc_raw+=("${pid}${PROC_DELIM}${gpu_uuid}${PROC_DELIM}${used_raw}${PROC_DELIM}${used_mb}${PROC_DELIM}${used_known}${PROC_DELIM}${proc_name}")
    pids+=("$pid")
  done <<< "$proc_lines"

  if (( ${#pids[@]} == 0 )); then
    reconcile_selection
    prune_selected_pids
    return
  fi

  local -A cmd_by_pid=()
  local pid_list
  pid_list="$(IFS=,; echo "${pids[*]}")"
  if [[ -n "$pid_list" ]]; then
    local ps_output
    ps_output="$(ps -o pid=,args= -ww -p "$pid_list" 2>/dev/null || ps -o pid=,args= -p "$pid_list" 2>/dev/null || true)"
    if [[ -n "$ps_output" ]]; then
      while IFS= read -r line; do
        line="$(trim "$line")"
        [[ -z "$line" ]] && continue
        local pid="${line%%[[:space:]]*}"
        local cmd="${line#"$pid"}"
        pid="$(trim "$pid")"
        cmd="$(trim "$cmd")"
        [[ -z "$pid" ]] && continue
        cmd_by_pid["$pid"]="$cmd"
      done <<< "$ps_output"
    fi
  fi

  local -A pmon_mem_by_pid=()
  local pmon_output
  pmon_output="$(nvidia-smi pmon -c 1 2>/dev/null || true)"
  if [[ -n "$pmon_output" ]]; then
    while IFS= read -r line; do
      line="$(trim "$line")"
      [[ -z "$line" ]] && continue
      [[ "${line:0:1}" == "#" ]] && continue
      local -a cols=()
      read -r -a cols <<< "$line"
      if (( ${#cols[@]} < 5 )); then
        continue
      fi
      local pid="${cols[1]}"
      local mem="${cols[4]}"
      if [[ "$pid" =~ ^[0-9]+$ && "$mem" =~ ^[0-9]+$ ]]; then
        pmon_mem_by_pid["$pid"]="$mem"
      fi
    done <<< "$pmon_output"
  fi

  for entry in "${proc_raw[@]}"; do
    local pid gpu_uuid used_raw used_mb used_known proc_name
    IFS="$PROC_DELIM" read -r pid gpu_uuid used_raw used_mb used_known proc_name <<< "$entry"
    if [[ "$used_known" -eq 0 ]]; then
      local pmon_mem="${pmon_mem_by_pid[$pid]:-}"
      if [[ -n "$pmon_mem" ]]; then
        used_raw="$pmon_mem"
        used_mb="$pmon_mem"
        used_known=1
      fi
    fi

    local gpu_idx="${GPU_INDEX_BY_UUID[$gpu_uuid]:-?}"
    local cmd="${cmd_by_pid[$pid]:-}"
    if [[ -z "$cmd" ]]; then
      cmd="$proc_name"
    fi

    PROC_USED_MB_BY_PID["$pid"]="$used_mb"
    PROC_USED_KNOWN_BY_PID["$pid"]="$used_known"
    PROC_USED_RAW_BY_PID["$pid"]="$used_raw"
    PROC_GPU_BY_PID["$pid"]="$gpu_idx"
    PROC_CMD_BY_PID["$pid"]="$cmd"
    PROC_NAME_BY_PID["$pid"]="$proc_name"
    if [[ "$used_known" -eq 1 ]]; then
      PROC_VRAM_AVAILABLE=1
    else
      PROC_VRAM_UNKNOWN=1
    fi

    local sort_key="$used_mb"
    if [[ "$used_known" -eq 0 ]]; then
      sort_key="-1"
    fi
    PROC_ENTRIES+=("${sort_key}${PROC_DELIM}${pid}")
  done

  if (( ${#PROC_ENTRIES[@]} == 0 )); then
    reconcile_selection
    prune_selected_pids
    return
  fi

  if [[ "$SORT_PROCS" -eq 1 ]]; then
    IFS=$'\n' PROC_ENTRIES=($(printf '%s\n' "${PROC_ENTRIES[@]}" | sort -t "$PROC_DELIM" -k1,1nr -k2,2n))
    unset IFS
  fi

  for entry in "${PROC_ENTRIES[@]}"; do
    local sort_key pid
    IFS="$PROC_DELIM" read -r sort_key pid <<< "$entry"
    PROC_PIDS+=("$pid")
  done
  PROC_COUNT=${#PROC_PIDS[@]}
  if (( PROC_COUNT > 0 )) && [[ "$PROC_VRAM_AVAILABLE" -eq 0 && "$PROC_VRAM_UNKNOWN" -eq 1 ]]; then
    PROC_SHOW_VRAM=0
  fi
  reconcile_selection
  prune_selected_pids
}

render_process_rows() {
  local rows=()
  local used_w="$PROC_W_USED"
  local bar_w="$PROC_W_BAR"
  local cmd_w="$PROC_W_CMD"
  if [[ "$PROC_SHOW_VRAM" -eq 0 ]]; then
    used_w=0
    bar_w=0
    local base=$((PROC_W_SEL + 1 + PROC_W_PID + 1 + PROC_W_GPU + 1))
    cmd_w=$((TERM_COLS - base))
    if (( cmd_w < 5 )); then
      cmd_w=5
    fi
  fi

  if [[ "$SHOW_PROCS" -eq 1 ]]; then
    local title="== GPU processes =="
    if [[ "$SORT_PROCS" -eq 1 ]]; then
      if [[ "$PROC_VRAM_AVAILABLE" -eq 1 ]]; then
        title="== GPU processes (sorted by VRAM alloc) =="
      else
        title="== GPU processes (sorted by PID; VRAM N/A) =="
      fi
    elif [[ "$PROC_COUNT" -gt 0 && "$PROC_VRAM_AVAILABLE" -eq 0 ]]; then
      title="== GPU processes (VRAM N/A) =="
    fi
    print_at "$PROC_SECTION_ROW" 0 "${COLOR_BOLD}${COLOR_CYAN}${title}${COLOR_RESET}"
  else
    print_at "$PROC_SECTION_ROW" 0 "${COLOR_BOLD}${COLOR_CYAN}== GPU processes (hidden) ==${COLOR_RESET}"
  fi

  if [[ "$SHOW_PROCS" -eq 0 ]]; then
    for ((i = 0; i < PROC_ROWS_SHOWN; i++)); do
      print_at "$((PROC_ROW_START + i))" 0 ""
    done
    print_at "$PROC_HEADER_ROW" 0 ""
    return
  fi

  local proc_header=""
  if [[ "$PROC_SHOW_VRAM" -eq 1 ]]; then
    proc_header="$(printf "${COLOR_BOLD}%-*s %-*s %-*s %-*s" "$PROC_W_SEL" "SEL" "$PROC_W_PID" "PID" "$PROC_W_GPU" "GPU" "$used_w" "VRAM Alloc(MB/GB)")"
    if (( bar_w > 0 )); then
      proc_header+=" $(fit_text "VRAM Alloc" "$bar_w")"
    fi
    proc_header+=" $(fit_text "COMMAND" "$cmd_w")${COLOR_RESET}"
  else
    proc_header="$(printf "${COLOR_BOLD}%-*s %-*s %-*s" "$PROC_W_SEL" "SEL" "$PROC_W_PID" "PID" "$PROC_W_GPU" "GPU")"
    proc_header+=" $(fit_text "COMMAND" "$cmd_w")${COLOR_RESET}"
  fi
  print_at "$PROC_HEADER_ROW" 0 "$(fit_text "$proc_header" "$TERM_COLS")"

  if (( PROC_COUNT == 0 )); then
    rows+=("${COLOR_WHITE}(no active compute processes)${COLOR_RESET}")
  else
    for i in "${!PROC_PIDS[@]}"; do
      local pid="${PROC_PIDS[$i]}"
      local gpu_idx="${PROC_GPU_BY_PID[$pid]:-?}"
      local used_mb="${PROC_USED_MB_BY_PID[$pid]:-0}"
      local used_known="${PROC_USED_KNOWN_BY_PID[$pid]:-0}"
      local used_raw="${PROC_USED_RAW_BY_PID[$pid]:-}"
      local cmd="${PROC_CMD_BY_PID[$pid]:-${PROC_NAME_BY_PID[$pid]:-}}"

      local used_label=""
      if [[ "$used_known" -eq 1 ]]; then
        local used_label_mb
        local used_label_gb
        used_label_mb="$(format_mb "$used_mb")"
        used_label_gb="$(format_gb "$used_mb")"
        used_label="${used_label_mb}MB/${used_label_gb}GB"
      else
        used_label="${used_raw:-N/A}"
      fi

      local cmd_display
      cmd_display="$(fit_text "$cmd" "$cmd_w")"

      local cursor=" "
      local chosen=" "
      if (( i == SELECTED_PROC_INDEX )); then
        cursor=">"
      fi
      if [[ -n "${SELECTED_PIDS[$pid]+x}" ]]; then
        chosen="*"
      fi
      local marker="${cursor}${chosen}"

      local line
      if [[ "$PROC_SHOW_VRAM" -eq 1 ]]; then
        local bar=""
        if (( bar_w > 0 )); then
          if [[ "$used_known" -eq 1 && "$gpu_idx" != "?" ]]; then
            local total_mb="${GPU_TOTAL_BY_INDEX[$gpu_idx]:-0}"
            bar="$(make_bar "$used_mb" "$total_mb" "$bar_w" "$COLOR_GREEN")"
          else
            bar="$(fit_text "" "$bar_w")"
          fi
        fi
        line="$(printf "%-*s ${COLOR_BOLD}%-*s${COLOR_RESET} %-*s ${COLOR_GREEN}%-*s${COLOR_RESET}" "$PROC_W_SEL" "$marker" "$PROC_W_PID" "$pid" "$PROC_W_GPU" "$gpu_idx" "$used_w" "$used_label")"
        if (( bar_w > 0 )); then
          line+=" $bar"
        fi
        line+=" $cmd_display"
      else
        line="$(printf "%-*s ${COLOR_BOLD}%-*s${COLOR_RESET} %-*s" "$PROC_W_SEL" "$marker" "$PROC_W_PID" "$pid" "$PROC_W_GPU" "$gpu_idx")"
        line+=" $cmd_display"
      fi
      if (( i == SELECTED_PROC_INDEX )); then
        line="${COLOR_REVERSE}${line}${COLOR_RESET}"
      fi
      rows+=("$line")
    done
  fi

  local row="$PROC_ROW_START"
  for ((i = 0; i < PROC_ROWS_SHOWN; i++)); do
    if [[ $i -lt ${#rows[@]} ]]; then
      print_at "$row" 0 "$(fit_text "${rows[$i]}" "$TERM_COLS")"
    else
      print_at "$row" 0 ""
    fi
    row=$((row + 1))
  done
}

render_selected_process_detail() {
  if [[ "$SHOW_PROCS" -eq 0 || "$PROC_COUNT" -eq 0 ]]; then
    print_at "$PROMPT_ROW" 0 ""
    return
  fi

  local pid="${PROC_PIDS[$SELECTED_PROC_INDEX]}"
  if [[ -z "$pid" ]]; then
    print_at "$PROMPT_ROW" 0 ""
    return
  fi

  local gpu_idx="${PROC_GPU_BY_PID[$pid]:-?}"
  local used_mb="${PROC_USED_MB_BY_PID[$pid]:-0}"
  local used_known="${PROC_USED_KNOWN_BY_PID[$pid]:-0}"
  local used_raw="${PROC_USED_RAW_BY_PID[$pid]:-}"
  local used_label=""
  if [[ "$used_known" -eq 1 ]]; then
    local used_label_mb
    local used_label_gb
    used_label_mb="$(format_mb "$used_mb")"
    used_label_gb="$(format_gb "$used_mb")"
    used_label="${used_label_mb}MB/${used_label_gb}GB"
  else
    used_label="${used_raw:-N/A}"
  fi

  local user=""
  local etime=""
  local cpu=""
  local mem=""
  local rss=""
  local ps_line
  ps_line="$(ps -o user=,etime=,pcpu=,pmem=,rss= -p "$pid" 2>/dev/null | sed 's/^[[:space:]]*//' || true)"
  if [[ -n "$ps_line" ]]; then
    read -r user etime cpu mem rss <<< "$ps_line"
  fi

  local rss_mb=""
  local rss_kb
  rss_kb="$(safe_int "$rss")"
  if (( rss_kb > 0 )); then
    rss_mb=$((rss_kb / 1024))
  fi

  local cmd="${PROC_CMD_BY_PID[$pid]:-${PROC_NAME_BY_PID[$pid]:-}}"
  local detail="Selected PID ${pid} | GPU ${gpu_idx} | VRAM ${used_label}"
  if [[ -n "$user" ]]; then
    detail+=" | User ${user} | CPU ${cpu}% | MEM ${mem}%"
    if [[ -n "$rss_mb" ]]; then
      detail+=" | RSS ${rss_mb}MB"
    fi
    detail+=" | ETIME ${etime}"
  fi
  if [[ -n "$cmd" ]]; then
    detail+=" | ${cmd}"
  fi
  print_at "$PROMPT_ROW" 0 "$(fit_text "${COLOR_BLUE}${detail}${COLOR_RESET}" "$TERM_COLS")"
}

move_selection() {
  local delta="$1"
  if [[ "$SHOW_PROCS" -eq 0 || "$PROC_COUNT" -eq 0 ]]; then
    return
  fi

  local next=$((SELECTED_PROC_INDEX + delta))
  if (( next < 0 )); then
    next=0
  fi
  if (( next >= PROC_COUNT )); then
    next=$((PROC_COUNT - 1))
  fi
  if (( next != SELECTED_PROC_INDEX )); then
    SELECTED_PROC_INDEX=$next
    CURSOR_PID="${PROC_PIDS[$SELECTED_PROC_INDEX]}"
  fi
}

toggle_selection() {
  local pid="$1"
  if [[ -z "$pid" ]]; then
    return
  fi
  if [[ -n "${SELECTED_PIDS[$pid]+x}" ]]; then
    unset 'SELECTED_PIDS[$pid]'
  else
    SELECTED_PIDS["$pid"]=1
  fi
}

update_process_rows() {
  load_process_entries
  render_process_rows
  render_selected_process_detail
}

prompt_kill() {
  local selected=()
  for pid in "${!SELECTED_PIDS[@]}"; do
    if [[ "$pid" =~ ^[0-9]+$ ]]; then
      selected+=("$pid")
    fi
  done
  if (( ${#selected[@]} > 0 )); then
    local pid_list
    pid_list="$(printf "%s " "${selected[@]}")"
    pid_list="${pid_list% }"
    print_at "$PROMPT_ROW" 0 "${COLOR_RED}Sending SIGTERM to selected:${COLOR_RESET} $pid_list"
    kill -- "${selected[@]}" || true
    sleep 1
    print_at "$PROMPT_ROW" 0 ""
    for pid in "${selected[@]}"; do
      unset 'SELECTED_PIDS[$pid]'
    done
    render_selected_process_detail
    return
  fi

  local prompt="Enter PIDs to kill (space-separated): "
  print_at "$PROMPT_ROW" 0 "$(fit_text "${COLOR_BLUE}${prompt}${COLOR_RESET}" "$TERM_COLS")"
  move_cursor "$PROMPT_ROW" ${#prompt}
  local pids=""
  if read -r -u "$INPUT_FD" pids; then
    print_at "$PROMPT_ROW" 0 ""
    if [[ -n "$pids" ]]; then
      local to_kill=()
      for pid in $pids; do
        if [[ "$pid" =~ ^[0-9]+$ ]]; then
          to_kill+=("$pid")
        fi
      done
      if (( ${#to_kill[@]} > 0 )); then
        local pid_list
        pid_list="$(printf "%s " "${to_kill[@]}")"
        pid_list="${pid_list% }"
        print_at "$PROMPT_ROW" 0 "${COLOR_RED}Sending SIGTERM to:${COLOR_RESET} $pid_list"
        kill -- "${to_kill[@]}" || true
        sleep 1
        print_at "$PROMPT_ROW" 0 ""
      else
        print_at "$PROMPT_ROW" 0 "${COLOR_RED}No valid PIDs entered.${COLOR_RESET}"
        sleep 1
        print_at "$PROMPT_ROW" 0 ""
      fi
    fi
  fi
  render_selected_process_detail
}

load_gpu_info
calc_layout
render_static

PAUSED=0
SHOW_PROCS="$SHOW_PROCS"

last_update=0
last_time=-1

while true; do
  if [[ "$NEEDS_REDRAW" -eq 1 ]]; then
    load_gpu_info
    calc_layout
    render_static
    NEEDS_REDRAW=0
  fi

  local_now="$(date +%s)"
  if [[ "$local_now" -ne "$last_time" ]]; then
    print_at "$LINE_TIME" 0 "${COLOR_BLUE}Time:${COLOR_RESET} $(date '+%Y-%m-%d %H:%M:%S')"
    last_time="$local_now"
  fi

  update_status "$PAUSED" "$SHOW_PROCS"

  if [[ "$PAUSED" -eq 0 && $((local_now - last_update)) -ge "$INTERVAL_SECONDS" ]]; then
    update_gpu_rows
    update_process_rows
    last_update="$local_now"
  fi

  if read -rsn1 -t "$TICK_SECONDS" -u "$INPUT_FD" key; then
    if [[ "$key" == $'\x1b' ]]; then
      if read -rsn2 -t 0.001 -u "$INPUT_FD" rest; then
        key+="$rest"
      fi
    fi
    case "$key" in
      $'\x1b[A'|$'\x1bOA')
        move_selection -1
        render_process_rows
        render_selected_process_detail
        ;;
      $'\x1b[B'|$'\x1bOB')
        move_selection 1
        render_process_rows
        render_selected_process_detail
        ;;
      ' ')
        if [[ "$SHOW_PROCS" -eq 1 && "$PROC_COUNT" -gt 0 ]]; then
          selected_pid="${PROC_PIDS[$SELECTED_PROC_INDEX]}"
          toggle_selection "$selected_pid"
          render_process_rows
          render_selected_process_detail
        fi
        ;;
      q|Q)
        print_at "$PROMPT_ROW" 0 ""
        exit 0
        ;;
      p|P)
        if [[ "$PAUSED" -eq 0 ]]; then
          PAUSED=1
        else
          PAUSED=0
        fi
        ;;
      t|T)
        if [[ "$SHOW_PROCS" -eq 0 ]]; then
          SHOW_PROCS=1
        else
          SHOW_PROCS=0
        fi
        update_process_rows
        ;;
      s|S)
        if [[ "$SORT_PROCS" -eq 0 ]]; then
          SORT_PROCS=1
        else
          SORT_PROCS=0
        fi
        update_process_rows
        ;;
      c|C)
        if [[ "$COMPACT" -eq 0 ]]; then
          COMPACT=1
        else
          COMPACT=0
        fi
        NEEDS_REDRAW=1
        ;;
      b|B)
        if [[ "$SHOW_BARS" -eq 0 ]]; then
          SHOW_BARS=1
        else
          SHOW_BARS=0
        fi
        NEEDS_REDRAW=1
        ;;
      k|K)
        prompt_kill
        ;;
    esac
  fi

done
