import struct, sys

d = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', d, 0x3c)[0]
nsec, stamp, opt = struct.unpack_from('<H', d, pe + 6)[0], struct.unpack_from('<I', d, pe + 8)[0], struct.unpack_from('<H', d, pe + 20)[0]
secs = [struct.unpack_from('<8xIIII', d, pe + 24 + opt + 40 * i) for i in range(nsec)]
off = lambda r: next(r - va + rp for vs, va, rs, rp in secs if va <= r < va + max(vs, rs))
e = off(struct.unpack_from('<I', d, pe + 24 + 112)[0])
count, funcs, names, ords = struct.unpack_from('<IIII', d, e + 24)
want = {b'fake_crash': 'Fake_Crash_Here', b'fake_heap_free': 'Fake_Heap_Free',
        b'fake_acclim_refresh': 'Game_PracticeTrackAcclimatisation_RefreshState',
        b'fake_acclim_flag': 'Game_PracticeTrackAcclimatisation_FlagCurrentTarget',
        b'fake_powertrain_ers': 'Car_Powertrain_Step',
        b'fake_standings_weekend': 'Career_Standings_BuildWeekendNode',
        b'fake_weekends_record': 'Career_Weekends_BuildResultNodes',
        b'fake_teamhq_node': 'Career_TeamHqAdvance_BuildTrackNode',
        b'fake_teammate_pairs': 'Sess_Strategy_UpdateTeammatePairs',
        b'fake_pairs_sort': 'Sess_Strategy_SortTeamCars',
        b'fake_setup_vm': 'Game_CarSetupVm_Initialise',
        b'fake_list_first_id': 'FeTile_Get_ListFirstEntrantId',
        b'fake_list_usable': 'FeTile_Check_EntrantListUsable',
        b'fake_ai_corner': 'Ai_Job_UpdateCarsParallel',
        b'fake_replay_write': 'Ne_Replay_WriteFrames',
        b'fake_lb_select': 'Game_TimeTrialLeaderboard_SelectBoard',
        b'fake_mtv_load': 'Game_Load_MyTeamVehicleBases',
        b'fake_mtv_ai': 'Sess_Load_VehicleAiConfig',
        b'fake_mtv_save': 'Game_Save_MyTeamVehicleBases',
        b'fake_sv_classes': 'Sess_Build_SessionVehicleClassLists',
        b'fake_mtv_replace': 'Ent_EntrantsSystem_ReplaceVehicle'}
rows = []
for i in range(count):
    name = d[off(struct.unpack_from('<I', d, off(names) + 4 * i)[0]):].split(b'\0')[0]
    if name in want:
        rva = struct.unpack_from('<I', d, off(funcs) + 4 * struct.unpack_from('<H', d, off(ords) + 2 * i)[0])[0]
        rows.append(f'{rva:x}\t{want[name]}\tL0\n')
open(sys.argv[2], 'w').write(f'# F1_25.exe fake {stamp}\n' + ''.join(sorted(rows)))
