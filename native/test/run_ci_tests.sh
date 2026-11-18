#!/usr/bin/env bash
# Runs CI's native tests side by side, NATIVE_TEST_JOBS at a time (default: one per core). A line reports each
# test as it ends; then each test's output follows in a log group, in the order below, longest first.
# The Wine tests need Wine and an xvfb-run that gives every call its own display, as ci.yml sets up; the
# stock xvfb-run -a can hand two calls started together the same one. Tests given the same lock name run
# one at a time.
set -uo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
export ZIG_GLOBAL_CACHE_DIR=$ROOT/native/build/zig-cache ZIG_LOCAL_CACHE_DIR=$ROOT/native/build/zig-cache

NAMES=()
CMDS=()
LOCKS=()
t() { NAMES+=("$1"); CMDS+=("$2"); LOCKS+=("${3:-}"); }

zig_test() { echo "zig c++ -target x86_64-linux-musl -std=c++23 -O2 $1 -o $2 && ./$2"; }

# A synthetic F1_25.exe carries the signatures; no game files are used.
t "Wine test" native/test/run_wine_test.sh port-21414
t "Registry core (shared corpus, Python reference, random stacks, read benchmark)" native/test/run_registry_test.sh
t "Standard library 1.6 headers compile test" native/test/run_sdk_headers_test.sh
t "Schema check unit and agreement test" native/test/run_schema_test.sh
t "Test agent Wine test" native/test/run_testagent_test.sh
# Builds every templates/std addon with its native/build.sh and validates its registry data files.
t "Standard library templates" "python3 tools/std/templates.py"
t "Events dispatcher unit test" native/test/run_events_test.sh
t "Content registries (row engine, Classic Seasons lowering, variants, stat-id pools)" native/test/run_content_test.sh
# Without work/exe/F1_25.exe it runs the synthetic packed image only.
t "Binding resolver Wine test" native/test/run_bind_test.sh
t "Career operations planner and SDK calls host test" native/test/run_career_ops_test.sh
t "swofty::json unit test" native/test/run_json_test.sh
t "Loader build gate unit test" native/test/run_build_gate_test.sh
t "Live apply paths (scheduler, put-back, context, budgets, sizing, stat ids, reports)" native/test/run_apply_test.sh
t "Track registries (seeds, record lowering, parity with the launcher's track activation)" native/test/run_tracks_test.sh
t "Save sandbox host and Wine test" native/test/run_save_sandbox_test.sh
t "Runtime assets Wine test" native/test/run_runtime_assets_test.sh
t "Box track lists (fit rules, calendar and More tracks, per-box entries, fuel plan, guard decision)" native/test/run_track_lists_test.sh
t "Career snapshots (swofty_registry entry, freezes, budgets, migration, fixtures)" native/test/run_snapshots_test.sh
t "Socket trace Wine test" native/test/run_net_trace_test.sh
t "Career UI Wine test" native/test/run_career_ui_test.sh
t "Fast-fail crash reports (fast-fail codes, reasons, exit statuses, byte patterns)" native/test/run_fast_fail_test.sh
t "Loader log fallback Wine test" native/test/run_log_fallback_test.sh
t "String override Wine test" native/test/run_lang_override_test.sh
t "String scope unit test" native/test/run_lang_scope_test.sh
t "Overlay toolkit Wine test" native/test/run_toolkit_test.sh
t "Overlay draw-list unit test" native/test/run_hud_test.sh
t "HUD leaf table unit test" native/test/run_hud_leaves_test.sh
t "CSV recorder unit test" native/test/run_log_csv_test.sh
t "HUD runtime headers test" native/test/run_hud_osd_test.sh
t "Live to timing tower adapter test" native/test/run_timing_live_test.sh
t "HUD dial template host test" templates/hud-dial/native/test/run_test.sh
# The 1.6 handler guard, built as build.sh builds it, against faults and SDK handlers that throw.
t "Handler guard Wine test" native/test/run_guard_test.sh
t "Memory page checks Wine test" native/test/run_mem_pages_test.sh
t "Settings read memo Wine test" native/test/run_settings_get_test.sh
t "Career list Wine test" native/test/run_career_list_test.sh
t "Hook points, events and services host test" native/test/run_hooks_test.sh
t "Game hook points host test" native/test/run_game_points_test.sh
t "Session control, AI controller and spectator points host test" native/test/run_session_points_test.sh
t "Live car operations host test" native/test/run_car_live_test.sh
t "Hook points review test" native/test/run_w5_review_test.sh
t "Shader look unit test" native/test/run_grade_test.sh
t "Post-process pass logic unit test" native/test/run_postfx_test.sh
t "Shader rules engine unit test" native/test/run_rules_test.sh
t "Live car signal mapping unit test" native/test/run_live_car_test.sh
t "Live state memory reads, derivations and telemetry fill" native/test/run_live_test.sh
t "ERS new-lap site unit test" native/test/run_ers_newlap_test.sh
t "Round track record guard unit test" native/test/run_track_record_test.sh
t "Team HQ track guard unit test" native/test/run_teamhq_track_test.sh
t "Career calendar track guard unit test" native/test/run_calendar_track_test.sh
t "Activity resume guard unit test" native/test/run_activity_resume_test.sh
t "Events sequencer record guard unit test" native/test/run_sequencer_records_test.sh
t "Race retirement driver guard unit test" native/test/run_race_retirement_test.sh
t "Standings dropped scores unit test" native/test/run_standings_drop_test.sh
t "AI team curves guard unit test" native/test/run_ai_curves_test.sh
t "Teammate pairs guard unit test" native/test/run_teammate_pairs_test.sh
t "R&D supplier budget unit test" native/test/run_rnd_suppliers_test.sh
t "Asset group template archives unit test" native/test/run_asset_template_test.sh
t "Setup screen car guard unit test" native/test/run_setup_vm_test.sh
t "AI brake line guard unit test" native/test/run_ai_corner_test.sh
t "Replay recorder grid guard unit test" native/test/run_replay_grid_test.sh
t "placed record bound unit test" native/test/run_placed_record_test.sh
t "Tyre slot guard unit test" native/test/run_tyre_slots_test.sh
t "Tyre model check unit test" native/test/run_tyre_models_test.sh
t "Pit slot guard unit test" native/test/run_pit_slots_test.sh
t "Pit box guard unit test" native/test/run_pit_box_test.sh
t "Garage slot guard unit test" native/test/run_garage_slots_test.sh
t "Direct start unit test" native/test/run_direct_start_test.sh
t "Direct start Time Trial unit test" native/test/run_direct_start_tt_test.sh
t "Time Trial loaded list board and team label unit test" native/test/run_tt_loaded_test.sh
t "Thrustmaster LCD guard unit test" native/test/run_thrustmaster_lcd_test.sh
t "Estimate lap time guard unit test" native/test/run_estimate_laptime_test.sh
t "Red flag recompute guard unit test" native/test/run_red_flag_recompute_test.sh
t "Career calendar track list guard unit test" native/test/run_track_list_guard_test.sh
t "Garage setup guard unit test" native/test/run_garage_setup_test.sh
t "Theatre list guard unit test" native/test/run_theatre_list_test.sh
t "Tile list select guard unit test" native/test/run_tile_list_select_test.sh
t "Core site names exported (host-only)" native/test/run_core_sites_exported_test.sh
t "Garage crew guard unit test" native/test/run_garage_crew_test.sh
t "Garage placement and crew poses" "bash native/test/run_garage_placement_test.sh"
t "Garage crew guard sites export test" native/test/run_garage_crew_sites_test.sh
t "Grand Prix continue guard unit test" native/test/run_gp_continue_test.sh
t "Lap toast message and state machine unit test" native/test/run_toast_test.sh
t "Player name sanitising unit test" native/test/run_player_name_test.sh
t "UI registry walk unit test" native/test/run_ui_registry_walk_test.sh
t "Leaderboard guard unit test" native/test/run_leaderboard_guard_test.sh
t "Memory scan helpers unit test" native/test/run_mem_scan_test.sh
t "Front-end tile list guard unit test" native/test/run_fe_tile_guard_test.sh
t "Stock game message log unit test" native/test/run_game_message_log_test.sh
t "My Team vehicle base guard unit test" native/test/run_myteam_vehicle_guard_test.sh
t "My Team vehicle heal unit test" native/test/run_myteam_vehicle_heal_test.sh
t "My Team vehicle keep unit test" native/test/run_myteam_vehicle_keep_test.sh
t "Main menu logic unit test" native/test/run_main_menu_test.sh
t "Grand Prix remembered-track fallback unit test" native/test/run_gp_track_test.sh
t "Weekend track guard unit test" native/test/run_gp_weekend_track_test.sh
t "Session enter guard unit test" native/test/run_session_enter_test.sh
t "AI line set guard unit test" native/test/run_ai_line_set_test.sh
t "Audio job queue fix unit test" native/test/run_audio_queue_test.sh
t "Shader resource view clamp unit test" native/test/run_srv_clamp_test.sh
t "Ray tracing build guard unit test" native/test/run_rtas_guard_test.sh
t "Heap size table unit test" native/test/run_heap_sizes_test.sh
t "Profiler minute summary unit test" native/test/run_perf_logic_test.sh
t "Frame pacing logic unit test" native/test/run_pacing_logic_test.sh
t "Save temp heap size unit test" native/test/run_save_heap_test.sh
t "Boot watchdog grace unit test" native/test/run_watchdog_grace_test.sh
t "Load-stall watchdog trigger unit test" native/test/run_load_stall_test.sh
t "Load-stall report running-threads order unit test" native/test/run_stall_order_test.sh
t "Session job list scan probe unit test" native/test/run_session_job_probe_test.sh
t "Pattern scan and scan cache unit test" native/test/run_scan_test.sh
t "Telemetry feed unit test" "$(zig_test "-I packages/sdk/include native/test/telemetry_feed_test.cpp" telemetry_feed_test)"
t "Save diagnostics log formatting unit test" "$(zig_test "-I packages/sdk/include native/test/save_diag_test.cpp" save_diag_test)"

JOBS=${NATIVE_TEST_JOBS:-$(nproc)}
LOGS=$(mktemp -d)
echo "${#CMDS[@]} tests, $JOBS at a time"
for i in "${!CMDS[@]}"; do
    while [ "$(jobs -rp | wc -l)" -ge "$JOBS" ]; do
        wait -n
    done
    (
        start=$SECONDS
        if [ -n "${LOCKS[$i]}" ]; then
            flock "$LOGS/${LOCKS[$i]}.lock" bash -c "${CMDS[$i]}"
        else
            bash -c "${CMDS[$i]}"
        fi >"$LOGS/$i.log" 2>&1
        rc=$?
        echo "$rc $((SECONDS - start))" >"$LOGS/$i.rc"
        echo "$([ $rc = 0 ] && echo "ok  " || echo FAIL) $((SECONDS - start))s ${NAMES[$i]}"
    ) &
done
wait

failed=()
for i in "${!CMDS[@]}"; do
    read -r rc secs <"$LOGS/$i.rc" 2>/dev/null || { rc=1 secs=?; }
    echo "::group::$([ "$rc" = 0 ] && echo ok || echo FAILED) ${secs}s ${NAMES[$i]}"
    cat "$LOGS/$i.log"
    echo "::endgroup::"
    [ "$rc" = 0 ] || failed+=("${NAMES[$i]}")
done
for name in "${failed[@]}"; do
    echo "::error title=Native test failed::$name"
done
[ ${#failed[@]} -eq 0 ]
