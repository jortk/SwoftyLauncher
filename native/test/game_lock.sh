# Sourced by test scripts that start a fake F1_25.exe under Wine. On a machine with the shared
# game lock (/mnt/work/F1/f1-game.lock.d, or $SWOFTY_GAME_LOCK), the script holds it for its whole
# run, so a real launch never sees a stray F1_25.exe. It takes the lock only while the game queue
# (f1-game.queue beside the lock, or $SWOFTY_GAME_QUEUE) is empty, so a queued game launch goes
# first. CI has no lock folder and skips this.
game_lock_take() {
    local lock=${SWOFTY_GAME_LOCK:-/mnt/work/F1/f1-game.lock.d}
    [ -n "${SWOFTY_GAME_LOCK_HELD:-}" ] && return 0
    [ -d "$(dirname "$lock")" ] || return 0
    local who="wine-test $(basename "$0") pid $$ $(date +%H:%M)" waited=0
    local queue=${SWOFTY_GAME_QUEUE:-$(dirname "$lock")/f1-game.queue}
    until [ ! -s "$queue" ] && mkdir "$lock" 2>/dev/null; do
        if [ $waited -eq 0 ]; then
            if [ -s "$queue" ]; then
                echo "game lock: $(wc -l < "$queue") run(s) queued in $queue; waiting until the queue is empty" >&2
            else
                echo "game lock: held by $(cat "$lock/owner" 2>/dev/null || echo unknown); waiting" >&2
            fi
        fi
        waited=$((waited + 5))
        if [ $waited -ge "${SWOFTY_GAME_LOCK_WAIT:-3600}" ]; then
            echo "game lock: still held after ${waited}s; giving up" >&2
            exit 3
        fi
        sleep 5
    done
    echo "$who" > "$lock/owner"
    export SWOFTY_GAME_LOCK_HELD=$lock
    trap 'rm -rf "$SWOFTY_GAME_LOCK_HELD"' EXIT
}
game_lock_take
