#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct BankAccount {
    uint8_t _pad_0x0[0x10];
    BankSubAccount *subs_begin; // L2 experimental
    BankSubAccount *subs_end; // L2 experimental
    uint8_t _pad_0x20[0x8];
    uint32_t key; // L2 experimental
    uint8_t _pad_0x2c[0x4];
};
static_assert(offsetof(BankAccount, subs_begin) == 0x10);
static_assert(offsetof(BankAccount, subs_end) == 0x18);
static_assert(offsetof(BankAccount, key) == 0x28);
static_assert(sizeof(BankAccount) == 0x30);

struct BankSubAccount {
    uint8_t _pad_0x0[0xc];
    uint8_t category; // L2 experimental
    uint8_t _pad_0xd[0x3];
    int32_t balance; // L2 experimental
    int32_t minimum; // L2 experimental
    uint8_t _pad_0x18[0x28];
    BankTransaction *history_begin; // L2 experimental
    BankTransaction *history_end; // L2 experimental
    uint8_t _pad_0x50[0x160];
};
static_assert(offsetof(BankSubAccount, category) == 0xc);
static_assert(offsetof(BankSubAccount, balance) == 0x10);
static_assert(offsetof(BankSubAccount, minimum) == 0x14);
static_assert(offsetof(BankSubAccount, history_begin) == 0x40);
static_assert(offsetof(BankSubAccount, history_end) == 0x48);
static_assert(sizeof(BankSubAccount) == 0x1b0);

struct BankTransaction {
    uint8_t type; // L2 experimental
    uint8_t _pad_0x1[0x3];
    int32_t amount; // L2 experimental
    uint8_t _pad_0x8[0x4];
    uint32_t time; // L2 experimental
    uint8_t flow; // L2 experimental
    uint8_t _pad_0x11[0x3];
};
static_assert(offsetof(BankTransaction, type) == 0x0);
static_assert(offsetof(BankTransaction, amount) == 0x4);
static_assert(offsetof(BankTransaction, time) == 0xc);
static_assert(offsetof(BankTransaction, flow) == 0x10);
static_assert(sizeof(BankTransaction) == 0x14);

struct BankTransactionMessage {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x8];
    uint16_t account; // L2 experimental
    uint8_t _pad_0x12[0x2];
    uint8_t type; // L2 experimental
    uint8_t _pad_0x15[0x3];
    int32_t amount; // L2 experimental
    uint32_t created; // L2 experimental
    uint32_t time; // L2 experimental
    uint8_t flow; // L2 experimental
    uint8_t check_funds; // L2 experimental
    uint8_t _pad_0x26[0x2];
    uint8_t category; // L2 experimental
};
static_assert(offsetof(BankTransactionMessage, vtable) == 0x0);
static_assert(offsetof(BankTransactionMessage, account) == 0x10);
static_assert(offsetof(BankTransactionMessage, type) == 0x14);
static_assert(offsetof(BankTransactionMessage, amount) == 0x18);
static_assert(offsetof(BankTransactionMessage, created) == 0x1c);
static_assert(offsetof(BankTransactionMessage, time) == 0x20);
static_assert(offsetof(BankTransactionMessage, flow) == 0x24);
static_assert(offsetof(BankTransactionMessage, check_funds) == 0x25);
static_assert(offsetof(BankTransactionMessage, category) == 0x28);

struct CarNumberComponent {
    uint8_t _pad_0x0[0x278];
    CareerEntity *entity; // L2 experimental
    uint8_t _pad_0x280[0x38];
};
static_assert(offsetof(CarNumberComponent, entity) == 0x278);
static_assert(sizeof(CarNumberComponent) == 0x2b8);

struct CareerBankComponent {
    uint8_t _pad_0x0[0x298];
    BankAccount *team_accounts_begin; // L2 experimental
    BankAccount *team_accounts_end; // L2 experimental
};
static_assert(offsetof(CareerBankComponent, team_accounts_begin) == 0x298);
static_assert(offsetof(CareerBankComponent, team_accounts_end) == 0x2a0);

struct CareerDriverContractRenewalComponent {
    uint8_t _pad_0x0[0x3a1];
    uint8_t disable_driver_moves; // L2 experimental
    uint8_t _pad_0x3a2[0x6];
    void *driver_moves_allocator; // L2 experimental
    DriverMoveRecord *driver_moves_begin; // L2 experimental
    DriverMoveRecord *driver_moves_end; // L2 experimental
    uint64_t driver_moves_capacity; // L2 experimental
};
static_assert(offsetof(CareerDriverContractRenewalComponent, disable_driver_moves) == 0x3a1);
static_assert(offsetof(CareerDriverContractRenewalComponent, driver_moves_allocator) == 0x3a8);
static_assert(offsetof(CareerDriverContractRenewalComponent, driver_moves_begin) == 0x3b0);
static_assert(offsetof(CareerDriverContractRenewalComponent, driver_moves_end) == 0x3b8);
static_assert(offsetof(CareerDriverContractRenewalComponent, driver_moves_capacity) == 0x3c0);

struct CareerEntity {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x2d8];
    EntrantRecord *entrants_begin; // L2 experimental
    EntrantRecord *entrants_end; // L2 experimental
    uint8_t _pad_0x2f0[0x8];
    uint8_t entrant_update_busy; // L2 experimental
    uint8_t _pad_0x2f9[0xc5f];
    TrackDataSystem *track_data; // L2 experimental
    uint8_t _pad_0xf60[0xd0];
    void **component_registry; // L2 experimental
    uint8_t _pad_0x1038[0x8];
    void *championships_begin; // L2 experimental
    void *championships_end; // L2 experimental
    uint8_t _pad_0x1050[0x8];
    void *current_championship; // L2 experimental
    uint8_t _pad_0x1060[0x8];
    void **championship_order_begin; // L2 experimental
    void **championship_order_end; // L2 experimental
};
static_assert(offsetof(CareerEntity, vtable) == 0x0);
static_assert(offsetof(CareerEntity, entrants_begin) == 0x2e0);
static_assert(offsetof(CareerEntity, entrants_end) == 0x2e8);
static_assert(offsetof(CareerEntity, entrant_update_busy) == 0x2f8);
static_assert(offsetof(CareerEntity, track_data) == 0xf58);
static_assert(offsetof(CareerEntity, component_registry) == 0x1030);
static_assert(offsetof(CareerEntity, championships_begin) == 0x1040);
static_assert(offsetof(CareerEntity, championships_end) == 0x1048);
static_assert(offsetof(CareerEntity, current_championship) == 0x1058);
static_assert(offsetof(CareerEntity, championship_order_begin) == 0x1068);
static_assert(offsetof(CareerEntity, championship_order_end) == 0x1070);

struct CareerSponsorsComponent {
    uint8_t _pad_0x0[0x298];
    SponsorTeamRecord *teams_begin; // L2 experimental
    SponsorTeamRecord *teams_end; // L2 experimental
};
static_assert(offsetof(CareerSponsorsComponent, teams_begin) == 0x298);
static_assert(offsetof(CareerSponsorsComponent, teams_end) == 0x2a0);

struct CareerTimeComponent {
    uint8_t _pad_0x0[0x290];
    uint8_t season_dates; // L2 experimental
    uint8_t _pad_0x291[0x1f];
    uint32_t current_minutes; // L2 experimental
};
static_assert(offsetof(CareerTimeComponent, season_dates) == 0x290);
static_assert(offsetof(CareerTimeComponent, current_minutes) == 0x2b0);

struct Championship {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x200];
    void **list_vtable; // L2 experimental
    uint8_t _pad_0x210[0x328];
    uint32_t completion; // L2 experimental
    uint8_t _pad_0x53c[0xc];
    ChampionshipSeasonEntry *seasons_begin; // L2 experimental
    ChampionshipSeasonEntry *seasons_end; // L2 experimental
    uint8_t _pad_0x558[0x8];
    uint32_t season_index; // L2 experimental
    uint8_t _pad_0x564[0x4c];
};
static_assert(offsetof(Championship, vtable) == 0x0);
static_assert(offsetof(Championship, list_vtable) == 0x208);
static_assert(offsetof(Championship, completion) == 0x538);
static_assert(offsetof(Championship, seasons_begin) == 0x548);
static_assert(offsetof(Championship, seasons_end) == 0x550);
static_assert(offsetof(Championship, season_index) == 0x560);
static_assert(sizeof(Championship) == 0x5b0);

struct ChampionshipSeasonEntry {
    uint8_t _pad_0x0[0x8];
    Season *season; // L2 experimental
};
static_assert(offsetof(ChampionshipSeasonEntry, season) == 0x8);
static_assert(sizeof(ChampionshipSeasonEntry) == 0x10);

struct ContextStatSet {
    uint8_t _pad_0x0[0x10];
    DriverStatistic *stats_begin; // L2 experimental
    DriverStatistic *stats_end; // L2 experimental
    uint8_t _pad_0x20[0x8];
    uint32_t mask_a; // L2 experimental
    uint32_t mask_b; // L2 experimental
    uint8_t _pad_0x30[0x8];
};
static_assert(offsetof(ContextStatSet, stats_begin) == 0x10);
static_assert(offsetof(ContextStatSet, stats_end) == 0x18);
static_assert(offsetof(ContextStatSet, mask_a) == 0x28);
static_assert(offsetof(ContextStatSet, mask_b) == 0x2c);
static_assert(sizeof(ContextStatSet) == 0x38);

struct ContractRecord {
    uint8_t _pad_0x0[0x28];
    uint8_t clause_stat; // L2 experimental
    uint8_t _pad_0x29[0x3];
    int32_t clause_target; // L2 experimental
    uint8_t _pad_0x30[0x4];
    int32_t salary; // L2 experimental
    int32_t signing_bonus; // L2 experimental
    int32_t team_perf_money; // L2 experimental
    int32_t team_perf_race_value; // L2 experimental
    float success_chance; // L2 experimental
    int32_t negotiation_bonus; // L2 experimental
    uint8_t is_counteroffer; // L2 experimental
    uint8_t _pad_0x4d[0x3];
    uint32_t driver; // L2 experimental
    uint32_t old_team; // L2 experimental
    uint32_t new_team; // L2 experimental
    uint8_t driver_move_reason; // L2 experimental
    uint8_t _pad_0x5d[0x3];
    uint32_t weekend_index; // L2 experimental
    uint16_t career_day_index; // L2 experimental
    uint8_t _pad_0x66[0x2];
    uint16_t day_contract_sent; // L2 experimental
    uint16_t day_contract_decided; // L2 experimental
    uint8_t accepted; // L2 experimental
    uint8_t acknowledged; // L2 experimental
    uint8_t _pad_0x6e[0x2];
};
static_assert(offsetof(ContractRecord, clause_stat) == 0x28);
static_assert(offsetof(ContractRecord, clause_target) == 0x2c);
static_assert(offsetof(ContractRecord, salary) == 0x34);
static_assert(offsetof(ContractRecord, signing_bonus) == 0x38);
static_assert(offsetof(ContractRecord, team_perf_money) == 0x3c);
static_assert(offsetof(ContractRecord, team_perf_race_value) == 0x40);
static_assert(offsetof(ContractRecord, success_chance) == 0x44);
static_assert(offsetof(ContractRecord, negotiation_bonus) == 0x48);
static_assert(offsetof(ContractRecord, is_counteroffer) == 0x4c);
static_assert(offsetof(ContractRecord, driver) == 0x50);
static_assert(offsetof(ContractRecord, old_team) == 0x54);
static_assert(offsetof(ContractRecord, new_team) == 0x58);
static_assert(offsetof(ContractRecord, driver_move_reason) == 0x5c);
static_assert(offsetof(ContractRecord, weekend_index) == 0x60);
static_assert(offsetof(ContractRecord, career_day_index) == 0x64);
static_assert(offsetof(ContractRecord, day_contract_sent) == 0x68);
static_assert(offsetof(ContractRecord, day_contract_decided) == 0x6a);
static_assert(offsetof(ContractRecord, accepted) == 0x6c);
static_assert(offsetof(ContractRecord, acknowledged) == 0x6d);
static_assert(sizeof(ContractRecord) == 0x70);

struct CostCapComponent {
    uint8_t _pad_0x0[0x2d8];
    CostCapTeam **team_slots; // L2 experimental
    uint8_t *team_ctrl; // L2 experimental
};
static_assert(offsetof(CostCapComponent, team_slots) == 0x2d8);
static_assert(offsetof(CostCapComponent, team_ctrl) == 0x2e0);

struct CostCapSeason {
    int32_t season_id; // L2 experimental
    uint8_t _pad_0x4[0x4];
    int32_t cached_expenses; // L2 experimental
    int32_t cost_cap; // L2 experimental
    float cached_modifier; // L2 experimental
};
static_assert(offsetof(CostCapSeason, season_id) == 0x0);
static_assert(offsetof(CostCapSeason, cached_expenses) == 0x8);
static_assert(offsetof(CostCapSeason, cost_cap) == 0xc);
static_assert(offsetof(CostCapSeason, cached_modifier) == 0x10);

struct CostCapTeam {
    uint16_t account; // L2 experimental
    uint8_t _pad_0x2[0x2e];
    CostCapSeason **season_slots; // L2 experimental
    uint8_t *season_ctrl; // L2 experimental
};
static_assert(offsetof(CostCapTeam, account) == 0x0);
static_assert(offsetof(CostCapTeam, season_slots) == 0x30);
static_assert(offsetof(CostCapTeam, season_ctrl) == 0x38);

struct CurvePoint {
    float x; // L2 experimental
    float y; // L2 experimental
};
static_assert(offsetof(CurvePoint, x) == 0x0);
static_assert(offsetof(CurvePoint, y) == 0x4);
static_assert(sizeof(CurvePoint) == 0x8);

struct DriverAcademiesComponent {
    uint8_t _pad_0x0[0x298];
    void *academies_begin; // L2 experimental
    void *academies_end; // L2 experimental
    uint8_t _pad_0x2a8[0x20];
    void *driver_academy_entries; // L2 experimental
};
static_assert(offsetof(DriverAcademiesComponent, academies_begin) == 0x298);
static_assert(offsetof(DriverAcademiesComponent, academies_end) == 0x2a0);
static_assert(offsetof(DriverAcademiesComponent, driver_academy_entries) == 0x2c8);

struct DriverAcademyMapSlot {
    uint64_t index; // L2 experimental
    uint32_t status; // L2 experimental
};
static_assert(offsetof(DriverAcademyMapSlot, index) == 0x0);
static_assert(offsetof(DriverAcademyMapSlot, status) == 0x8);

struct DriverCareerContractOwner {
    uint8_t _pad_0x0[0x298];
    void *contracts_begin; // L2 experimental
    void *contracts_end; // L2 experimental
};
static_assert(offsetof(DriverCareerContractOwner, contracts_begin) == 0x298);
static_assert(offsetof(DriverCareerContractOwner, contracts_end) == 0x2a0);

struct DriverCareerContractsManagerComponent {
    uint8_t _pad_0x0[0x270];
    void *weekend_source; // L2 experimental
    CareerEntity *entity; // L2 experimental
    uint8_t _pad_0x280[0xd0];
    DriverCareerContractOwner *contract_owner; // L2 experimental
    CareerTimeComponent *career_time; // L2 experimental
    uint8_t _pad_0x360[0x8];
    CarNumberComponent *car_numbers; // L2 experimental
    ScheduleComponent *schedule; // L2 experimental
    uint8_t _pad_0x378[0x30];
    void *secret_meetings_begin; // L2 experimental
    void *secret_meetings_end; // L2 experimental
};
static_assert(offsetof(DriverCareerContractsManagerComponent, weekend_source) == 0x270);
static_assert(offsetof(DriverCareerContractsManagerComponent, entity) == 0x278);
static_assert(offsetof(DriverCareerContractsManagerComponent, contract_owner) == 0x350);
static_assert(offsetof(DriverCareerContractsManagerComponent, career_time) == 0x358);
static_assert(offsetof(DriverCareerContractsManagerComponent, car_numbers) == 0x368);
static_assert(offsetof(DriverCareerContractsManagerComponent, schedule) == 0x370);
static_assert(offsetof(DriverCareerContractsManagerComponent, secret_meetings_begin) == 0x3a8);
static_assert(offsetof(DriverCareerContractsManagerComponent, secret_meetings_end) == 0x3b0);

struct DriverContract {
    uint32_t team; // L2 experimental
    uint8_t _pad_0x4[0x8];
    uint16_t signed_day; // L2 experimental
    uint8_t kind; // L2 experimental
    uint8_t _pad_0xf[0x1];
    uint16_t end_day; // L2 experimental
    uint8_t state; // L2 experimental
    uint8_t _pad_0x13[0x1];
    uint32_t end_value; // L2 experimental
    uint8_t _pad_0x18[0x14];
    uint8_t active; // L2 experimental
    uint8_t _pad_0x2d[0xb];
    void *history_begin; // L2 experimental
    void *history_end; // L2 experimental
    uint8_t _pad_0x48[0x8];
    uint32_t driver; // L2 experimental
};
static_assert(offsetof(DriverContract, team) == 0x0);
static_assert(offsetof(DriverContract, signed_day) == 0xc);
static_assert(offsetof(DriverContract, kind) == 0xe);
static_assert(offsetof(DriverContract, end_day) == 0x10);
static_assert(offsetof(DriverContract, state) == 0x12);
static_assert(offsetof(DriverContract, end_value) == 0x14);
static_assert(offsetof(DriverContract, active) == 0x2c);
static_assert(offsetof(DriverContract, history_begin) == 0x38);
static_assert(offsetof(DriverContract, history_end) == 0x40);
static_assert(offsetof(DriverContract, driver) == 0x50);

struct DriverContractChangedMessage {
    void **vtable; // L2 experimental
    uint64_t reserved; // L2 experimental
    uint32_t driver; // L2 experimental
    uint8_t before[48]; // L2 experimental
    uint8_t after[44]; // L2 experimental
    uint8_t kind; // L2 experimental
    uint8_t _pad_0x71[0x7];
};
static_assert(offsetof(DriverContractChangedMessage, vtable) == 0x0);
static_assert(offsetof(DriverContractChangedMessage, reserved) == 0x8);
static_assert(offsetof(DriverContractChangedMessage, driver) == 0x10);
static_assert(offsetof(DriverContractChangedMessage, before) == 0x14);
static_assert(offsetof(DriverContractChangedMessage, after) == 0x44);
static_assert(offsetof(DriverContractChangedMessage, kind) == 0x70);
static_assert(sizeof(DriverContractChangedMessage) == 0x78);

struct DriverContractsMyTeamComponent {
    uint8_t _pad_0x0[0x2e8];
    CarNumberComponent *car_numbers; // L2 experimental
    uint8_t _pad_0x2f0[0x4a8];
    ContractRecord *pending_contracts_begin; // L2 experimental
    uint8_t _pad_0x7a0[0x18];
    ContractRecord *decided_contracts_begin; // L2 experimental
    uint8_t _pad_0x7c0[0x18];
    ContractRecord *pending_moves_begin; // L2 experimental
    ContractRecord *pending_moves_end; // L2 experimental
    uint8_t _pad_0x7e8[0x10];
    ContractRecord *last_season_signings_begin; // L2 experimental
    ContractRecord *last_season_signings_end; // L2 experimental
};
static_assert(offsetof(DriverContractsMyTeamComponent, car_numbers) == 0x2e8);
static_assert(offsetof(DriverContractsMyTeamComponent, pending_contracts_begin) == 0x798);
static_assert(offsetof(DriverContractsMyTeamComponent, decided_contracts_begin) == 0x7b8);
static_assert(offsetof(DriverContractsMyTeamComponent, pending_moves_begin) == 0x7d8);
static_assert(offsetof(DriverContractsMyTeamComponent, pending_moves_end) == 0x7e0);
static_assert(offsetof(DriverContractsMyTeamComponent, last_season_signings_begin) == 0x7f8);
static_assert(offsetof(DriverContractsMyTeamComponent, last_season_signings_end) == 0x800);

struct DriverMoveRecord {
    uint32_t driver; // L2 experimental
    uint32_t old_team; // L2 experimental
    uint32_t new_team; // L2 experimental
    uint8_t reason; // L2 experimental
    uint8_t _pad_0xd[0x3];
    uint32_t weekend_index; // L2 experimental
    uint16_t day_index; // L2 experimental
    uint8_t _pad_0x16[0x2];
};
static_assert(offsetof(DriverMoveRecord, driver) == 0x0);
static_assert(offsetof(DriverMoveRecord, old_team) == 0x4);
static_assert(offsetof(DriverMoveRecord, new_team) == 0x8);
static_assert(offsetof(DriverMoveRecord, reason) == 0xc);
static_assert(offsetof(DriverMoveRecord, weekend_index) == 0x10);
static_assert(offsetof(DriverMoveRecord, day_index) == 0x14);
static_assert(sizeof(DriverMoveRecord) == 0x18);

struct DriverMoveConfirmedMessage {
    void **vtable; // L2 experimental
    uint64_t reserved; // L2 experimental
    DriverMoveRecord move; // L2 experimental
};
static_assert(offsetof(DriverMoveConfirmedMessage, vtable) == 0x0);
static_assert(offsetof(DriverMoveConfirmedMessage, reserved) == 0x8);
static_assert(offsetof(DriverMoveConfirmedMessage, move) == 0x10);
static_assert(sizeof(DriverMoveConfirmedMessage) == 0x28);

struct LineupChange {
    uint32_t driver; // L2 experimental
    uint32_t old_team; // L2 experimental
    uint32_t new_team; // L2 experimental
    uint8_t _pad_0xc[0x4];
};
static_assert(offsetof(LineupChange, driver) == 0x0);
static_assert(offsetof(LineupChange, old_team) == 0x4);
static_assert(offsetof(LineupChange, new_team) == 0x8);
static_assert(sizeof(LineupChange) == 0x10);

struct DriverMovesFinalisedMessage {
    void **vtable; // L2 experimental
    uint64_t reserved; // L2 experimental
    LineupChange changes[40]; // L2 experimental
    uint64_t count; // L2 experimental
};
static_assert(offsetof(DriverMovesFinalisedMessage, vtable) == 0x0);
static_assert(offsetof(DriverMovesFinalisedMessage, reserved) == 0x8);
static_assert(offsetof(DriverMovesFinalisedMessage, changes) == 0x10);
static_assert(offsetof(DriverMovesFinalisedMessage, count) == 0x290);
static_assert(sizeof(DriverMovesFinalisedMessage) == 0x298);

struct EvolutionCurve {
    void **vtable; // L2 experimental
    float inv_x_range; // L2 experimental
    float min_x; // L2 experimental
    float y_step; // L2 experimental
    float end_value; // L2 experimental
    uint8_t _pad_0x18[0x4];
    float min_y; // L2 experimental
    float max_y; // L2 experimental
    uint8_t lut[65]; // L2 experimental
    uint8_t _pad_0x65[0x3];
    void *allocator; // L2 experimental
    CurvePoint *points_begin; // L2 experimental
    uint8_t _pad_0x78[0x8];
    uint64_t points_capacity; // L2 experimental
};
static_assert(offsetof(EvolutionCurve, vtable) == 0x0);
static_assert(offsetof(EvolutionCurve, inv_x_range) == 0x8);
static_assert(offsetof(EvolutionCurve, min_x) == 0xc);
static_assert(offsetof(EvolutionCurve, y_step) == 0x10);
static_assert(offsetof(EvolutionCurve, end_value) == 0x14);
static_assert(offsetof(EvolutionCurve, min_y) == 0x1c);
static_assert(offsetof(EvolutionCurve, max_y) == 0x20);
static_assert(offsetof(EvolutionCurve, lut) == 0x24);
static_assert(offsetof(EvolutionCurve, allocator) == 0x68);
static_assert(offsetof(EvolutionCurve, points_begin) == 0x70);
static_assert(offsetof(EvolutionCurve, points_capacity) == 0x80);
static_assert(sizeof(EvolutionCurve) == 0x88);

struct DriverStatistic {
    uint8_t _pad_0x0[0x10];
    uint16_t info_stat_id; // L2 experimental
    uint8_t _pad_0x12[0x6];
    EvolutionCurve evolution_graph; // L2 experimental
    EvolutionCurve patched_evolution_graph; // L2 experimental
    uint8_t has_patched_graph; // L2 experimental
    uint8_t _pad_0x129[0x7];
    float temporary_boost_value; // L2 experimental
    uint8_t is_dynamic_stat; // L2 experimental
    uint8_t _pad_0x135[0x3];
    float dynamic_level; // L2 experimental
    uint8_t _pad_0x13c[0x4];
};
static_assert(offsetof(DriverStatistic, info_stat_id) == 0x10);
static_assert(offsetof(DriverStatistic, evolution_graph) == 0x18);
static_assert(offsetof(DriverStatistic, patched_evolution_graph) == 0xa0);
static_assert(offsetof(DriverStatistic, has_patched_graph) == 0x128);
static_assert(offsetof(DriverStatistic, temporary_boost_value) == 0x130);
static_assert(offsetof(DriverStatistic, is_dynamic_stat) == 0x134);
static_assert(offsetof(DriverStatistic, dynamic_level) == 0x138);
static_assert(sizeof(DriverStatistic) == 0x140);

struct DriverStatisticsComponent {
    uint8_t _pad_0x0[0x278];
    void *stat_context; // L2 experimental
    uint8_t _pad_0x280[0x10];
    CareerTimeComponent *career_time; // L2 experimental
    uint8_t _pad_0x298[0x10];
    uint8_t stats_data; // L2 experimental
    uint8_t use_story_mode_data; // L2 experimental
    uint8_t _pad_0x2aa[0x2];
    uint32_t last_patch_applied_checksum; // L2 experimental
    uint32_t last_patch_declined_checksum; // L2 experimental
    uint8_t _pad_0x2b4[0x1c];
    void *stat_infos_begin; // L2 experimental
    void *stat_infos_end; // L2 experimental
    uint8_t _pad_0x2e0[0x30];
    DriverStatsEntry *driver_data_begin; // L2 experimental
    DriverStatsEntry *driver_data_end; // L2 experimental
};
static_assert(offsetof(DriverStatisticsComponent, stat_context) == 0x278);
static_assert(offsetof(DriverStatisticsComponent, career_time) == 0x290);
static_assert(offsetof(DriverStatisticsComponent, stats_data) == 0x2a8);
static_assert(offsetof(DriverStatisticsComponent, use_story_mode_data) == 0x2a9);
static_assert(offsetof(DriverStatisticsComponent, last_patch_applied_checksum) == 0x2ac);
static_assert(offsetof(DriverStatisticsComponent, last_patch_declined_checksum) == 0x2b0);
static_assert(offsetof(DriverStatisticsComponent, stat_infos_begin) == 0x2d0);
static_assert(offsetof(DriverStatisticsComponent, stat_infos_end) == 0x2d8);
static_assert(offsetof(DriverStatisticsComponent, driver_data_begin) == 0x310);
static_assert(offsetof(DriverStatisticsComponent, driver_data_end) == 0x318);

struct DriverStatsEntry {
    uint8_t kind; // L2 experimental
    uint8_t _pad_0x1[0x1];
    uint16_t stat_id; // L2 experimental
    uint8_t _pad_0x4[0x14];
    DriverStatistic *stats_begin; // L2 experimental
    DriverStatistic *stats_end; // L2 experimental
    uint8_t _pad_0x28[0x10];
    ContextStatSet *context_sets_begin; // L2 experimental
    ContextStatSet *context_sets_end; // L2 experimental
    uint8_t _pad_0x48[0x8];
};
static_assert(offsetof(DriverStatsEntry, kind) == 0x0);
static_assert(offsetof(DriverStatsEntry, stat_id) == 0x2);
static_assert(offsetof(DriverStatsEntry, stats_begin) == 0x18);
static_assert(offsetof(DriverStatsEntry, stats_end) == 0x20);
static_assert(offsetof(DriverStatsEntry, context_sets_begin) == 0x38);
static_assert(offsetof(DriverStatsEntry, context_sets_end) == 0x40);
static_assert(sizeof(DriverStatsEntry) == 0x50);

struct F1SeasonStandings {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x18];
    StandingsRound *points_begin; // L2 experimental
    StandingsRound *points_end; // L2 experimental
    uint8_t _pad_0x30[0x78];
    StandingsPoints *extra_points_begin; // L2 experimental
    StandingsPoints *extra_points_end; // L2 experimental
    uint8_t _pad_0xb8[0x18];
    StandingsRound *results_begin; // L2 experimental
    StandingsRound *results_end; // L2 experimental
};
static_assert(offsetof(F1SeasonStandings, vtable) == 0x0);
static_assert(offsetof(F1SeasonStandings, points_begin) == 0x20);
static_assert(offsetof(F1SeasonStandings, points_end) == 0x28);
static_assert(offsetof(F1SeasonStandings, extra_points_begin) == 0xa8);
static_assert(offsetof(F1SeasonStandings, extra_points_end) == 0xb0);
static_assert(offsetof(F1SeasonStandings, results_begin) == 0xd0);
static_assert(offsetof(F1SeasonStandings, results_end) == 0xd8);

struct FacilityNode {
    void **vtable; // L2 experimental
    uint8_t state; // L2 experimental
    uint8_t _pad_0x9[0x3];
    uint32_t stat_id; // L2 experimental
    uint8_t _pad_0x10[0x10];
    void *children_begin; // L2 experimental
    void *children_end; // L2 experimental
    uint8_t _pad_0x30[0x10];
    int32_t phase; // L2 experimental
    uint8_t _pad_0x44[0x3c];
    int32_t cost; // L2 experimental
    uint8_t _pad_0x84[0x4];
    char *name_loc_tag; // L2 experimental
    uint8_t _pad_0x90[0x18];
    int32_t days_to_complete; // L2 experimental
    int32_t days_remaining; // L2 experimental
    uint8_t purchased; // L2 experimental
    uint8_t _pad_0xb1[0x2];
    uint8_t seen_completed; // L2 experimental
    uint8_t _pad_0xb4[0x28];
    int32_t ui_level; // L2 experimental
    int32_t ui_max_level; // L2 experimental
    uint8_t _pad_0xe4[0x4];
};
static_assert(offsetof(FacilityNode, vtable) == 0x0);
static_assert(offsetof(FacilityNode, state) == 0x8);
static_assert(offsetof(FacilityNode, stat_id) == 0xc);
static_assert(offsetof(FacilityNode, children_begin) == 0x20);
static_assert(offsetof(FacilityNode, children_end) == 0x28);
static_assert(offsetof(FacilityNode, phase) == 0x40);
static_assert(offsetof(FacilityNode, cost) == 0x80);
static_assert(offsetof(FacilityNode, name_loc_tag) == 0x88);
static_assert(offsetof(FacilityNode, days_to_complete) == 0xa8);
static_assert(offsetof(FacilityNode, days_remaining) == 0xac);
static_assert(offsetof(FacilityNode, purchased) == 0xb0);
static_assert(offsetof(FacilityNode, seen_completed) == 0xb3);
static_assert(offsetof(FacilityNode, ui_level) == 0xdc);
static_assert(offsetof(FacilityNode, ui_max_level) == 0xe0);
static_assert(sizeof(FacilityNode) == 0xe8);

struct FacilityPerkTree {
    uint8_t _pad_0x0[0x8];
    void *nodes_begin; // L2 experimental
    void *nodes_end; // L2 experimental
    uint8_t _pad_0x18[0x10];
};
static_assert(offsetof(FacilityPerkTree, nodes_begin) == 0x8);
static_assert(offsetof(FacilityPerkTree, nodes_end) == 0x10);
static_assert(sizeof(FacilityPerkTree) == 0x28);

struct FacilityTeamData {
    uint8_t _pad_0x0[0x10];
    void *facilities_begin; // L2 experimental
    void *facilities_end; // L2 experimental
    uint8_t _pad_0x20[0x8];
    uint32_t team_key; // L2 experimental
    uint8_t _pad_0x2c[0x54];
};
static_assert(offsetof(FacilityTeamData, facilities_begin) == 0x10);
static_assert(offsetof(FacilityTeamData, facilities_end) == 0x18);
static_assert(offsetof(FacilityTeamData, team_key) == 0x28);
static_assert(sizeof(FacilityTeamData) == 0x80);

struct FacilityTeamEntry {
    uint8_t _pad_0x0[0x8];
    FacilityTeamData *data; // L2 experimental
};
static_assert(offsetof(FacilityTeamEntry, data) == 0x8);

struct MyTeamFacilitiesComponent {
    uint8_t _pad_0x0[0x2b8];
    FacilityTeamEntry **team_slots; // L2 experimental
    uint8_t *team_ctrl; // L2 experimental
    uint64_t team_count; // L2 experimental
};
static_assert(offsetof(MyTeamFacilitiesComponent, team_slots) == 0x2b8);
static_assert(offsetof(MyTeamFacilitiesComponent, team_ctrl) == 0x2c0);
static_assert(offsetof(MyTeamFacilitiesComponent, team_count) == 0x2c8);

struct MyTeamFacility {
    uint8_t facility_type; // L2 experimental
    uint8_t _pad_0x1[0x7];
    FacilityPerkTree *tree; // L2 experimental
    uint8_t _pad_0x10[0x40];
    int32_t workforce_cap; // L2 experimental
    uint8_t workforce_cap_set; // L2 experimental
    uint8_t _pad_0x55[0x183];
    int32_t highest_phase_seen; // L2 experimental
    uint8_t _pad_0x1dc[0x1c];
};
static_assert(offsetof(MyTeamFacility, facility_type) == 0x0);
static_assert(offsetof(MyTeamFacility, tree) == 0x8);
static_assert(offsetof(MyTeamFacility, workforce_cap) == 0x50);
static_assert(offsetof(MyTeamFacility, workforce_cap_set) == 0x54);
static_assert(offsetof(MyTeamFacility, highest_phase_seen) == 0x1d8);
static_assert(sizeof(MyTeamFacility) == 0x1f8);

struct RenownAwardedMessage {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x8];
    uint32_t team; // L2 experimental
    float amount; // L2 experimental
};
static_assert(offsetof(RenownAwardedMessage, vtable) == 0x0);
static_assert(offsetof(RenownAwardedMessage, team) == 0x10);
static_assert(offsetof(RenownAwardedMessage, amount) == 0x14);

struct RenownConfig {
    uint8_t _pad_0x0[0x8];
    uint32_t *tiers_begin; // L2 experimental
    uint32_t *tiers_end; // L2 experimental
};
static_assert(offsetof(RenownConfig, tiers_begin) == 0x8);
static_assert(offsetof(RenownConfig, tiers_end) == 0x10);

struct ResourcePointsComponent {
    uint8_t _pad_0x0[0x4a8];
    CareerEntity *wallet_entity; // L2 experimental
    uint8_t wallet_mode; // L2 experimental
    uint8_t _pad_0x4b1[0x7];
    void *driver_wallets; // L2 experimental
    uint8_t _pad_0x4c0[0x68];
    void *team_wallets; // L2 experimental
};
static_assert(offsetof(ResourcePointsComponent, wallet_entity) == 0x4a8);
static_assert(offsetof(ResourcePointsComponent, wallet_mode) == 0x4b0);
static_assert(offsetof(ResourcePointsComponent, driver_wallets) == 0x4b8);
static_assert(offsetof(ResourcePointsComponent, team_wallets) == 0x528);

struct RivalManager {
    uint8_t _pad_0x0[0x18];
    void *current_rivalries_begin; // L2 experimental
    void *current_rivalries_end; // L2 experimental
};
static_assert(offsetof(RivalManager, current_rivalries_begin) == 0x18);
static_assert(offsetof(RivalManager, current_rivalries_end) == 0x20);

struct RivalManagerComponent {
    uint8_t _pad_0x0[0xbd8];
    void *managers_begin; // L2 experimental
    void *managers_end; // L2 experimental
};
static_assert(offsetof(RivalManagerComponent, managers_begin) == 0xbd8);
static_assert(offsetof(RivalManagerComponent, managers_end) == 0xbe0);

struct Season {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x218];
    char *name; // L2 experimental
    uint8_t _pad_0x228[0x58];
    NeAllocator *allocator; // L2 experimental
    Weekend **weekends_begin; // L2 experimental
    Weekend **weekends_end; // L2 experimental
    uint8_t _pad_0x298[0x8];
    uint32_t current_weekend; // L2 experimental
    uint8_t _pad_0x2a4[0x4];
    void **standings_vtable; // L2 experimental
    uint8_t _pad_0x2b0[0x288];
    uint32_t completion_state; // L2 experimental
    uint8_t _pad_0x53c[0x14];
};
static_assert(offsetof(Season, vtable) == 0x0);
static_assert(offsetof(Season, name) == 0x220);
static_assert(offsetof(Season, allocator) == 0x280);
static_assert(offsetof(Season, weekends_begin) == 0x288);
static_assert(offsetof(Season, weekends_end) == 0x290);
static_assert(offsetof(Season, current_weekend) == 0x2a0);
static_assert(offsetof(Season, standings_vtable) == 0x2a8);
static_assert(offsetof(Season, completion_state) == 0x538);
static_assert(sizeof(Season) == 0x550);

struct Session {
    uint8_t _pad_0x0[0x8];
    uint32_t session_id; // L2 experimental
    uint8_t _pad_0xc[0x4];
    uint32_t completion; // L2 experimental
    uint8_t _pad_0x14[0x4];
    NeAllocator *participants_allocator; // L2 experimental
    EntrantRecord *participants_begin; // L2 experimental
    EntrantRecord *participants_end; // L2 experimental
    uint64_t participants_capacity; // L2 experimental
    uint8_t _pad_0x38[0x68];
};
static_assert(offsetof(Session, session_id) == 0x8);
static_assert(offsetof(Session, completion) == 0x10);
static_assert(offsetof(Session, participants_allocator) == 0x18);
static_assert(offsetof(Session, participants_begin) == 0x20);
static_assert(offsetof(Session, participants_end) == 0x28);
static_assert(offsetof(Session, participants_capacity) == 0x30);
static_assert(sizeof(Session) == 0xa0);

struct SponsorEntry {
    uint8_t _pad_0x0[0x10];
    const char *name; // L2 experimental
    uint8_t _pad_0x18[0x10];
    uint16_t stat_id; // L2 experimental
    uint8_t _pad_0x2a[0x6e];
    SponsorLoyaltyLevel *levels_begin; // L2 experimental
    SponsorLoyaltyLevel *levels_end; // L2 experimental
    uint8_t _pad_0xa8[0x30];
    uint8_t slot; // L2 experimental
    uint8_t _pad_0xd9[0x3b];
    int32_t loyalty_xp; // L2 experimental
    uint8_t _pad_0x118[0x40];
    uint8_t active; // L2 experimental
    uint8_t _pad_0x159[0x7];
};
static_assert(offsetof(SponsorEntry, name) == 0x10);
static_assert(offsetof(SponsorEntry, stat_id) == 0x28);
static_assert(offsetof(SponsorEntry, levels_begin) == 0x98);
static_assert(offsetof(SponsorEntry, levels_end) == 0xa0);
static_assert(offsetof(SponsorEntry, slot) == 0xd8);
static_assert(offsetof(SponsorEntry, loyalty_xp) == 0x114);
static_assert(offsetof(SponsorEntry, active) == 0x158);
static_assert(sizeof(SponsorEntry) == 0x160);

struct SponsorLoyaltyLevel {
    uint8_t claimed; // L2 experimental
    uint8_t _pad_0x1[0x3];
    int32_t xp_threshold; // L2 experimental
    uint8_t _pad_0x8[0x70];
};
static_assert(offsetof(SponsorLoyaltyLevel, claimed) == 0x0);
static_assert(offsetof(SponsorLoyaltyLevel, xp_threshold) == 0x4);
static_assert(sizeof(SponsorLoyaltyLevel) == 0x78);

struct SponsorLoyaltyXpMessage {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x8];
    uint32_t team; // L2 experimental
    int32_t amount; // L2 experimental
    uint8_t reason; // L2 experimental
};
static_assert(offsetof(SponsorLoyaltyXpMessage, vtable) == 0x0);
static_assert(offsetof(SponsorLoyaltyXpMessage, team) == 0x10);
static_assert(offsetof(SponsorLoyaltyXpMessage, amount) == 0x14);
static_assert(offsetof(SponsorLoyaltyXpMessage, reason) == 0x18);

struct SponsorTeamRecord {
    uint8_t _pad_0x0[0x8];
    uint32_t team; // L2 experimental
    uint8_t _pad_0xc[0xc];
    SponsorEntry *sponsors_begin; // L2 experimental
    SponsorEntry *sponsors_end; // L2 experimental
    uint8_t _pad_0x28[0x60];
};
static_assert(offsetof(SponsorTeamRecord, team) == 0x8);
static_assert(offsetof(SponsorTeamRecord, sponsors_begin) == 0x18);
static_assert(offsetof(SponsorTeamRecord, sponsors_end) == 0x20);
static_assert(sizeof(SponsorTeamRecord) == 0x88);

struct StandingsPoints {
    uint32_t key; // L2 experimental
    int32_t points; // L2 experimental
};
static_assert(offsetof(StandingsPoints, key) == 0x0);
static_assert(offsetof(StandingsPoints, points) == 0x4);
static_assert(sizeof(StandingsPoints) == 0x8);

struct StandingsResult {
    uint32_t key; // L2 experimental
    int32_t position; // L2 experimental
    int32_t fastest_lap_rank; // L2 experimental
    uint32_t status; // L2 experimental
};
static_assert(offsetof(StandingsResult, key) == 0x0);
static_assert(offsetof(StandingsResult, position) == 0x4);
static_assert(offsetof(StandingsResult, fastest_lap_rank) == 0x8);
static_assert(offsetof(StandingsResult, status) == 0xc);
static_assert(sizeof(StandingsResult) == 0x10);

struct StandingsRound {
    uint8_t _pad_0x0[0x8];
    StandingsSession *sessions_begin; // L2 experimental
    StandingsSession *sessions_end; // L2 experimental
    uint8_t _pad_0x18[0x8];
};
static_assert(offsetof(StandingsRound, sessions_begin) == 0x8);
static_assert(offsetof(StandingsRound, sessions_end) == 0x10);
static_assert(sizeof(StandingsRound) == 0x20);

struct StandingsSession {
    uint8_t _pad_0x0[0x8];
    void *records_begin; // L2 experimental
    void *records_end; // L2 experimental
    uint8_t _pad_0x18[0x8];
    uint32_t session_id; // L2 experimental
    uint8_t _pad_0x24[0x4];
};
static_assert(offsetof(StandingsSession, records_begin) == 0x8);
static_assert(offsetof(StandingsSession, records_end) == 0x10);
static_assert(offsetof(StandingsSession, session_id) == 0x20);
static_assert(sizeof(StandingsSession) == 0x28);

struct StatInfo {
    uint8_t _pad_0x0[0x8];
    uint16_t stat_id; // L2 experimental
    uint8_t _pad_0xa[0x126];
    float weight; // L2 experimental
    float default_value; // L2 experimental
};
static_assert(offsetof(StatInfo, stat_id) == 0x8);
static_assert(offsetof(StatInfo, weight) == 0x130);
static_assert(offsetof(StatInfo, default_value) == 0x134);

struct TeamRenownData {
    uint32_t team; // L2 experimental
    float points; // L2 experimental
    uint8_t _pad_0x8[0x10];
    RenownConfig *config; // L2 experimental
    uint8_t _pad_0x20[0x120];
};
static_assert(offsetof(TeamRenownData, team) == 0x0);
static_assert(offsetof(TeamRenownData, points) == 0x4);
static_assert(offsetof(TeamRenownData, config) == 0x18);
static_assert(sizeof(TeamRenownData) == 0x140);

struct UpgradeInstance {
    uint8_t _pad_0x0[0x10];
    char *name; // L2 experimental
    uint8_t _pad_0x18[0x40];
    uint32_t node_hash; // L2 experimental
    int32_t cost; // L2 experimental
    int32_t cost_to_develop; // L2 experimental
    uint8_t _pad_0x64[0x4];
    int32_t num_days_to_research; // L2 experimental
    int32_t num_days_to_develop; // L2 experimental
    uint8_t _pad_0x70[0x2];
    uint8_t department; // L2 experimental
    uint8_t _pad_0x73[0x1];
    int32_t unlock_at_facility_level; // L2 experimental
    uint8_t _pad_0x78[0x4];
    uint8_t row_flag; // L2 experimental
    uint8_t _pad_0x7d[0x1];
    uint16_t research_start_day; // L2 experimental
    uint16_t research_end_day; // L2 experimental
    uint8_t _pad_0x82[0x16];
    uint16_t develop_start_day; // L2 experimental
    uint16_t develop_end_day; // L2 experimental
    uint8_t _pad_0x9c[0x24];
    UpgradeStateEntry *states_begin; // L2 experimental
    UpgradeStateEntry *states_end; // L2 experimental
    uint8_t _pad_0xd0[0x10];
};
static_assert(offsetof(UpgradeInstance, name) == 0x10);
static_assert(offsetof(UpgradeInstance, node_hash) == 0x58);
static_assert(offsetof(UpgradeInstance, cost) == 0x5c);
static_assert(offsetof(UpgradeInstance, cost_to_develop) == 0x60);
static_assert(offsetof(UpgradeInstance, num_days_to_research) == 0x68);
static_assert(offsetof(UpgradeInstance, num_days_to_develop) == 0x6c);
static_assert(offsetof(UpgradeInstance, department) == 0x72);
static_assert(offsetof(UpgradeInstance, unlock_at_facility_level) == 0x74);
static_assert(offsetof(UpgradeInstance, row_flag) == 0x7c);
static_assert(offsetof(UpgradeInstance, research_start_day) == 0x7e);
static_assert(offsetof(UpgradeInstance, research_end_day) == 0x80);
static_assert(offsetof(UpgradeInstance, develop_start_day) == 0x98);
static_assert(offsetof(UpgradeInstance, develop_end_day) == 0x9a);
static_assert(offsetof(UpgradeInstance, states_begin) == 0xc0);
static_assert(offsetof(UpgradeInstance, states_end) == 0xc8);
static_assert(sizeof(UpgradeInstance) == 0xe0);

struct UpgradeStateEntry {
    uint32_t owner; // L2 experimental
    uint16_t day; // L2 experimental
    uint8_t _pad_0x6[0x2];
    uint8_t state; // L2 experimental
    uint8_t adaptation; // L2 experimental
    uint8_t _pad_0xa[0x2];
    uint8_t enabled; // L2 experimental
    uint8_t facility_locked; // L2 experimental
    uint8_t _pad_0xe[0x2];
};
static_assert(offsetof(UpgradeStateEntry, owner) == 0x0);
static_assert(offsetof(UpgradeStateEntry, day) == 0x4);
static_assert(offsetof(UpgradeStateEntry, state) == 0x8);
static_assert(offsetof(UpgradeStateEntry, adaptation) == 0x9);
static_assert(offsetof(UpgradeStateEntry, enabled) == 0xc);
static_assert(offsetof(UpgradeStateEntry, facility_locked) == 0xd);
static_assert(sizeof(UpgradeStateEntry) == 0x10);

struct UpgradeTree {
    uint8_t _pad_0x0[0x10];
    UpgradeInstance *instances_begin; // L2 experimental
    UpgradeInstance *instances_end; // L2 experimental
    uint8_t _pad_0x20[0xc20];
};
static_assert(offsetof(UpgradeTree, instances_begin) == 0x10);
static_assert(offsetof(UpgradeTree, instances_end) == 0x18);
static_assert(sizeof(UpgradeTree) == 0xc40);

struct Upgrades {
    uint8_t _pad_0x0[0x8];
    uint8_t mode; // L2 experimental
    uint8_t _pad_0x9[0x3];
    uint32_t team_key; // L2 experimental
    uint8_t _pad_0x10[0xd0];
    UpgradeTree trees[2]; // L2 experimental
    uint64_t tree_count; // L2 experimental
    uint8_t _pad_0x1968[0x140];
    uint32_t owner_cache; // L2 experimental
    uint8_t _pad_0x1aac[0x64];
};
static_assert(offsetof(Upgrades, mode) == 0x8);
static_assert(offsetof(Upgrades, team_key) == 0xc);
static_assert(offsetof(Upgrades, trees) == 0xe0);
static_assert(offsetof(Upgrades, tree_count) == 0x1960);
static_assert(offsetof(Upgrades, owner_cache) == 0x1aa8);
static_assert(sizeof(Upgrades) == 0x1b10);

struct UpgradesComponent {
    uint8_t _pad_0x0[0x298];
    Upgrades **upgrades_begin; // L2 experimental
    Upgrades **upgrades_end; // L2 experimental
    uint8_t _pad_0x2a8[0x18];
    CareerTimeComponent *career_time; // L2 experimental
};
static_assert(offsetof(UpgradesComponent, upgrades_begin) == 0x298);
static_assert(offsetof(UpgradesComponent, upgrades_end) == 0x2a0);
static_assert(offsetof(UpgradesComponent, career_time) == 0x2c0);

struct Weekend {
    void **vtable; // L2 experimental
    uint8_t _pad_0x8[0x20];
    char *name; // L2 experimental
    uint8_t _pad_0x30[0x1f0];
    Session *sessions_begin; // L2 experimental
    Session *sessions_end; // L2 experimental
    uint8_t _pad_0x230[0x8];
    uint32_t current_session; // L2 experimental
    uint32_t progress_state; // L2 experimental
    uint8_t _pad_0x240[0x38];
    uint8_t track_stat_id; // L2 experimental
    uint8_t _pad_0x279[0x277];
};
static_assert(offsetof(Weekend, vtable) == 0x0);
static_assert(offsetof(Weekend, name) == 0x28);
static_assert(offsetof(Weekend, sessions_begin) == 0x220);
static_assert(offsetof(Weekend, sessions_end) == 0x228);
static_assert(offsetof(Weekend, current_session) == 0x238);
static_assert(offsetof(Weekend, progress_state) == 0x23c);
static_assert(offsetof(Weekend, track_stat_id) == 0x278);
static_assert(sizeof(Weekend) == 0x4f0);

inline constexpr address<"Career_AcclaimRewardScreen_Ctor"> Career_AcclaimRewardScreen_Ctor{}; // L2 experimental
using Career_Add_SeasonWeekend_t = uint64_t(void *season, void *weekend);
inline constexpr fn<"Career_Add_SeasonWeekend", Career_Add_SeasonWeekend_t> Career_Add_SeasonWeekend{}; // L2 experimental
using Career_Add_TeamModifier_t = void(void *list, uint32_t key, float value);
inline constexpr fn<"Career_Add_TeamModifier", Career_Add_TeamModifier_t> Career_Add_TeamModifier{}; // L2 experimental
using Career_Adjust_RpDriverWallet_t = void(void *wallets, uint32_t driver_key, int32_t delta);
inline constexpr fn<"Career_Adjust_RpDriverWallet", Career_Adjust_RpDriverWallet_t> Career_Adjust_RpDriverWallet{}; // L2 experimental
using Career_Adjust_RpTeamWallet_t = void(void *wallets, uint32_t team_key, int32_t delta);
inline constexpr fn<"Career_Adjust_RpTeamWallet", Career_Adjust_RpTeamWallet_t> Career_Adjust_RpTeamWallet{}; // L2 experimental
inline constexpr address<"Career_Apply_RnDChallengerIconsLayout"> Career_Apply_RnDChallengerIconsLayout{}; // L2 experimental
using Career_Auction_ResolveAiOffer_t = void(void *self, void *teams, uint32_t a3, void *a4, bool a5, bool a6);
inline constexpr fn<"Career_Auction_ResolveAiOffer", Career_Auction_ResolveAiOffer_t> Career_Auction_ResolveAiOffer{}; // L2 experimental
inline constexpr address<"Career_Bank_ApplyTransactionToBalance"> Career_Bank_ApplyTransactionToBalance{}; // L2 experimental
using Career_Bank_CollectLedgerEntriesInRange_t = void *(void *bank, void *out_vec, uint32_t team_key, uint32_t time_from, uint32_t time_to);
inline constexpr fn<"Career_Bank_CollectLedgerEntriesInRange", Career_Bank_CollectLedgerEntriesInRange_t> Career_Bank_CollectLedgerEntriesInRange{}; // L2 experimental
using Career_Bank_OnBankAccountValidTransactionMessage_t = uint64_t(void *bank, void *msg);
inline constexpr fn<"Career_Bank_OnBankAccountValidTransactionMessage", Career_Bank_OnBankAccountValidTransactionMessage_t> Career_Bank_OnBankAccountValidTransactionMessage{}; // L2 experimental
inline constexpr address<"Career_Bank_ReadAccount"> Career_Bank_ReadAccount{}; // L2 experimental
inline constexpr address<"Career_Bank_ReadBudgetData"> Career_Bank_ReadBudgetData{}; // L2 experimental
inline constexpr address<"Career_Bank_ReadFacilityBudget"> Career_Bank_ReadFacilityBudget{}; // L2 experimental
inline constexpr address<"Career_Bank_ReadSaveData"> Career_Bank_ReadSaveData{}; // L2 experimental
inline constexpr address<"Career_Bank_UnregisterMessageHandlers"> Career_Bank_UnregisterMessageHandlers{}; // L2 experimental
inline constexpr address<"Career_Bank_WriteAccount"> Career_Bank_WriteAccount{}; // L2 experimental
inline constexpr address<"Career_Bank_WriteBudgetData"> Career_Bank_WriteBudgetData{}; // L2 experimental
inline constexpr address<"Career_Bank_WriteSaveData"> Career_Bank_WriteSaveData{}; // L2 experimental
inline constexpr address<"Career_Bind_ActivityFacilityIcons"> Career_Bind_ActivityFacilityIcons{}; // L2 experimental
inline constexpr address<"Career_Bind_DepartmentEventIcons"> Career_Bind_DepartmentEventIcons{}; // L2 experimental
using Career_Bind_DepartmentEventRewardArrows_t = uint64_t(int64_t *self, void *a2, void *a3);
inline constexpr fn<"Career_Bind_DepartmentEventRewardArrows", Career_Bind_DepartmentEventRewardArrows_t> Career_Bind_DepartmentEventRewardArrows{}; // L2 experimental
inline constexpr address<"Career_Bind_FacilityHeaderColours"> Career_Bind_FacilityHeaderColours{}; // L2 experimental
inline constexpr address<"Career_Bind_FacilityStatsPanel"> Career_Bind_FacilityStatsPanel{}; // L2 experimental
inline constexpr address<"Career_Bind_FacilityTileWidget"> Career_Bind_FacilityTileWidget{}; // L2 experimental
inline constexpr address<"Career_Bind_FacilityUpgradeNameStrings"> Career_Bind_FacilityUpgradeNameStrings{}; // L2 experimental
inline constexpr address<"Career_Bind_ResourcePointIcons"> Career_Bind_ResourcePointIcons{}; // L2 experimental
inline constexpr address<"Career_Bind_ResourcePointsIconsRnD"> Career_Bind_ResourcePointsIconsRnD{}; // L2 experimental
using Career_Bind_RnDAvailableNextPassPerfect_t = void(void *self, uint64_t *a2, uint64_t a3, void *a4);
inline constexpr fn<"Career_Bind_RnDAvailableNextPassPerfect", Career_Bind_RnDAvailableNextPassPerfect_t> Career_Bind_RnDAvailableNextPassPerfect{}; // L2 experimental
using Career_Bind_RnDDepartmentsTable_B_t = uint64_t(void *self);
inline constexpr fn<"Career_Bind_RnDDepartmentsTable_B", Career_Bind_RnDDepartmentsTable_B_t> Career_Bind_RnDDepartmentsTable_B{}; // L2 experimental
using Career_Bind_RnDOverview_t = uint64_t(void *self, int64_t *a2);
inline constexpr fn<"Career_Bind_RnDOverview", Career_Bind_RnDOverview_t> Career_Bind_RnDOverview{}; // L2 experimental
using Career_Bind_RnDUpgradesPanel_t = uint64_t(void *self);
inline constexpr fn<"Career_Bind_RnDUpgradesPanel", Career_Bind_RnDUpgradesPanel_t> Career_Bind_RnDUpgradesPanel{}; // L2 experimental
inline constexpr address<"Career_Bind_SpecialistUpgradeText"> Career_Bind_SpecialistUpgradeText{}; // L2 experimental
inline constexpr address<"Career_Bind_TeamHqPopupIcons"> Career_Bind_TeamHqPopupIcons{}; // L2 experimental
inline constexpr address<"Career_Build_ActivityCalendarFacilityColumn"> Career_Build_ActivityCalendarFacilityColumn{}; // L2 experimental
inline constexpr address<"Career_Build_DepartmentEventOutcomeView"> Career_Build_DepartmentEventOutcomeView{}; // L2 experimental
using Career_Build_DepartmentUpgradeCounts_t = void(void *self);
inline constexpr fn<"Career_Build_DepartmentUpgradeCounts", Career_Build_DepartmentUpgradeCounts_t> Career_Build_DepartmentUpgradeCounts{}; // L2 experimental
using Career_Build_DriverContract_t = void *(void *contracts, void *contract, uint32_t driver_key, uint32_t team_key, uint8_t type, int32_t target, int32_t seasons);
inline constexpr fn<"Career_Build_DriverContract", Career_Build_DriverContract_t> Career_Build_DriverContract{}; // L2 experimental
inline constexpr address<"Career_Build_FacilityUpgradeCompleteTimelineEvent"> Career_Build_FacilityUpgradeCompleteTimelineEvent{}; // L2 experimental
inline constexpr address<"Career_Build_FacilityUpgradeList"> Career_Build_FacilityUpgradeList{}; // L2 experimental
inline constexpr address<"Career_Build_FacilityUpgradeSlots"> Career_Build_FacilityUpgradeSlots{}; // L2 experimental
inline constexpr address<"Career_Build_FitUpgradeCardData"> Career_Build_FitUpgradeCardData{}; // L2 experimental
using Career_Build_LineupChanges_t = void *(void *out, const void *current_entrants, const void *baseline_entrants);
inline constexpr fn<"Career_Build_LineupChanges", Career_Build_LineupChanges_t> Career_Build_LineupChanges{}; // L2 experimental
inline constexpr address<"Career_Build_MyTeamSetupSummaryPage"> Career_Build_MyTeamSetupSummaryPage{}; // L2 experimental
using Career_Build_MyTeamUpgradePackUpsell_t = uint64_t(void *self);
inline constexpr fn<"Career_Build_MyTeamUpgradePackUpsell", Career_Build_MyTeamUpgradePackUpsell_t> Career_Build_MyTeamUpgradePackUpsell{}; // L2 experimental
using Career_Build_MyTeamUpgradesHeader_t = void(void *self);
inline constexpr fn<"Career_Build_MyTeamUpgradesHeader", Career_Build_MyTeamUpgradesHeader_t> Career_Build_MyTeamUpgradesHeader{}; // L2 experimental
inline constexpr address<"Career_Build_PerkResourcePointsBoostDescription"> Career_Build_PerkResourcePointsBoostDescription{}; // L2 experimental
inline constexpr address<"Career_Build_PerkResourcePointsBoostEntry"> Career_Build_PerkResourcePointsBoostEntry{}; // L2 experimental
using Career_Build_RnDBackground_A_t = void(int64_t *self, uint8_t *a2, uint64_t a3, void *a4);
inline constexpr fn<"Career_Build_RnDBackground_A", Career_Build_RnDBackground_A_t> Career_Build_RnDBackground_A{}; // L2 experimental
using Career_Build_RnDDeptIconStrip_t = uint64_t(void *self);
inline constexpr fn<"Career_Build_RnDDeptIconStrip", Career_Build_RnDDeptIconStrip_t> Career_Build_RnDDeptIconStrip{}; // L2 experimental
using Career_Build_RnDDevelopUpgradesCards_t = uint64_t(void *self);
inline constexpr fn<"Career_Build_RnDDevelopUpgradesCards", Career_Build_RnDDevelopUpgradesCards_t> Career_Build_RnDDevelopUpgradesCards{}; // L2 experimental
using Career_Build_RnDDevelopmentTopLevel_t = uint32_t *(void *self, uint64_t *a2, void *a3);
inline constexpr fn<"Career_Build_RnDDevelopmentTopLevel", Career_Build_RnDDevelopmentTopLevel_t> Career_Build_RnDDevelopmentTopLevel{}; // L2 experimental
using Career_Build_RnDDriverUpgradeLists_t = void(void *self);
inline constexpr fn<"Career_Build_RnDDriverUpgradeLists", Career_Build_RnDDriverUpgradeLists_t> Career_Build_RnDDriverUpgradeLists{}; // L2 experimental
using Career_Build_RnDDriverUpgradesView_t = int64_t *(int64_t *self, uint64_t *a2);
inline constexpr fn<"Career_Build_RnDDriverUpgradesView", Career_Build_RnDDriverUpgradesView_t> Career_Build_RnDDriverUpgradesView{}; // L2 experimental
using Career_Build_RnDFitUpgradesList_t = void(void *self);
inline constexpr fn<"Career_Build_RnDFitUpgradesList", Career_Build_RnDFitUpgradesList_t> Career_Build_RnDFitUpgradesList{}; // L2 experimental
using Career_Build_RnDFittedUpgradeTile_t = void(int64_t *self, uint8_t *a2, void *a3, uint8_t a4);
inline constexpr fn<"Career_Build_RnDFittedUpgradeTile", Career_Build_RnDFittedUpgradeTile_t> Career_Build_RnDFittedUpgradeTile{}; // L2 experimental
using Career_Build_RnDInProgressTile_t = void(void *self, char *a2);
inline constexpr fn<"Career_Build_RnDInProgressTile", Career_Build_RnDInProgressTile_t> Career_Build_RnDInProgressTile{}; // L2 experimental
using Career_Build_RnDPendingFitUpgradeBlock_t = void(int64_t *self, uint8_t *a2, void *a3, char a4);
inline constexpr fn<"Career_Build_RnDPendingFitUpgradeBlock", Career_Build_RnDPendingFitUpgradeBlock_t> Career_Build_RnDPendingFitUpgradeBlock{}; // L2 experimental
using Career_Build_RnDProgressHistory_t = void(void *self);
inline constexpr fn<"Career_Build_RnDProgressHistory", Career_Build_RnDProgressHistory_t> Career_Build_RnDProgressHistory{}; // L2 experimental
using Career_Build_RnDResearchUpgradeCard_t = uint64_t(int64_t *self, int64_t *a2);
inline constexpr fn<"Career_Build_RnDResearchUpgradeCard", Career_Build_RnDResearchUpgradeCard_t> Career_Build_RnDResearchUpgradeCard{}; // L2 experimental
using Career_Build_RnDResearchUpgradesHeader_t = void(void *self);
inline constexpr fn<"Career_Build_RnDResearchUpgradesHeader", Career_Build_RnDResearchUpgradesHeader_t> Career_Build_RnDResearchUpgradesHeader{}; // L2 experimental
using Career_Build_RnDTreeCard_t = int64_t *(int64_t *self, int64_t *a2);
inline constexpr fn<"Career_Build_RnDTreeCard", Career_Build_RnDTreeCard_t> Career_Build_RnDTreeCard{}; // L2 experimental
using Career_Build_RnDTreeView_t = void(void *self, uint64_t *a2);
inline constexpr fn<"Career_Build_RnDTreeView", Career_Build_RnDTreeView_t> Career_Build_RnDTreeView{}; // L2 experimental
using Career_Build_RnDUpgradeDetailPanel_t = void(void *self, uint8_t a2);
inline constexpr fn<"Career_Build_RnDUpgradeDetailPanel", Career_Build_RnDUpgradeDetailPanel_t> Career_Build_RnDUpgradeDetailPanel{}; // L2 experimental
using Career_Build_RnDUpgradeInfoCard_t = char *(void *self, uint64_t *a2);
inline constexpr fn<"Career_Build_RnDUpgradeInfoCard", Career_Build_RnDUpgradeInfoCard_t> Career_Build_RnDUpgradeInfoCard{}; // L2 experimental
using Career_Build_UpgradeCategoryList_t = void(void *self);
inline constexpr fn<"Career_Build_UpgradeCategoryList", Career_Build_UpgradeCategoryList_t> Career_Build_UpgradeCategoryList{}; // L2 experimental
inline constexpr address<"Career_Build_WorkforceAtCapTimelineEvent"> Career_Build_WorkforceAtCapTimelineEvent{}; // L2 experimental
inline constexpr address<"Career_Build_WorkforceCappedAdvanceCard"> Career_Build_WorkforceCappedAdvanceCard{}; // L2 experimental
inline constexpr address<"Career_Calc_CalendarDayIndex"> Career_Calc_CalendarDayIndex{}; // L2 experimental
inline constexpr address<"Career_Calc_DriverAgeYears"> Career_Calc_DriverAgeYears{}; // L2 experimental
using Career_Calc_DriverContractValueCurve_t = void(float x, uint32_t flags, void *team, int64_t *drv, float *out, void *table);
inline constexpr fn<"Career_Calc_DriverContractValueCurve", Career_Calc_DriverContractValueCurve_t> Career_Calc_DriverContractValueCurve{}; // L2 experimental
inline constexpr address<"Career_Calc_FacilityNodeScaledValueB"> Career_Calc_FacilityNodeScaledValueB{}; // L2 experimental
using Career_Calc_UpgradeCost_t = int32_t(void *list, void *inst, void *mods, uint32_t option);
inline constexpr fn<"Career_Calc_UpgradeCost", Career_Calc_UpgradeCost_t> Career_Calc_UpgradeCost{}; // L2 experimental
using Career_Calendar_AdvanceDays_t = void(void *clock_owner, uint16_t days, uint8_t flag_mask, char skip);
inline constexpr fn<"Career_Calendar_AdvanceDays", Career_Calendar_AdvanceDays_t> Career_Calendar_AdvanceDays{}; // L2 experimental
inline constexpr address<"Career_Calendar_IgnoreActivitiesInRange"> Career_Calendar_IgnoreActivitiesInRange{}; // L2 experimental
inline constexpr address<"Career_Calendar_IgnoreActivityById"> Career_Calendar_IgnoreActivityById{}; // L2 experimental
inline constexpr address<"Career_Calendar_MoveTrackData"> Career_Calendar_MoveTrackData{}; // L2 experimental
using Career_Cancel_UpgradeDevelopment_t = void(void *upgrades, uint32_t owner, void *inst, uint32_t day);
inline constexpr fn<"Career_Cancel_UpgradeDevelopment", Career_Cancel_UpgradeDevelopment_t> Career_Cancel_UpgradeDevelopment{}; // L2 experimental
using Career_Cancel_UpgradeResearch_t = void(void *upgrades, uint32_t owner, void *inst, uint32_t day);
inline constexpr fn<"Career_Cancel_UpgradeResearch", Career_Cancel_UpgradeResearch_t> Career_Cancel_UpgradeResearch{}; // L2 experimental
inline constexpr address<"Career_CareerOnlineDoc_ResetAfterSync"> Career_CareerOnlineDoc_ResetAfterSync{}; // L2 experimental
using Career_Check_DayHasNoBlockingEvent_t = bool(void *cal_owner, uint32_t day_key);
inline constexpr fn<"Career_Check_DayHasNoBlockingEvent", Career_Check_DayHasNoBlockingEvent_t> Career_Check_DayHasNoBlockingEvent{}; // L2 experimental
inline constexpr address<"Career_Check_DevelopmentTabHasAffordableUpgrade"> Career_Check_DevelopmentTabHasAffordableUpgrade{}; // L2 experimental
inline constexpr address<"Career_Check_DevelopmentTabHasPartsToStart"> Career_Check_DevelopmentTabHasPartsToStart{}; // L2 experimental
inline constexpr address<"Career_Check_LeftWithoutFacilityUpgrades"> Career_Check_LeftWithoutFacilityUpgrades{}; // L2 experimental
using Career_Check_PlayerHasUnspentWorkforceOrUpgrades_t = uint64_t(void *check, char flag);
inline constexpr fn<"Career_Check_PlayerHasUnspentWorkforceOrUpgrades", Career_Check_PlayerHasUnspentWorkforceOrUpgrades_t> Career_Check_PlayerHasUnspentWorkforceOrUpgrades{}; // L2 experimental
using Career_Check_ProfileEntryEqualsOne_t = bool(void *self);
inline constexpr fn<"Career_Check_ProfileEntryEqualsOne", Career_Check_ProfileEntryEqualsOne_t> Career_Check_ProfileEntryEqualsOne{}; // L2 experimental
using Career_Check_RecordSetContainsKind_t = bool(void *alloc, void *key, void *set_owner, void *record, char kind);
inline constexpr fn<"Career_Check_RecordSetContainsKind", Career_Check_RecordSetContainsKind_t> Career_Check_RecordSetContainsKind{}; // L2 experimental
inline constexpr address<"Career_Check_ResearchProjectReady"> Career_Check_ResearchProjectReady{}; // L2 experimental
inline constexpr address<"Career_Check_ResearchTabHasAvailableRp"> Career_Check_ResearchTabHasAvailableRp{}; // L2 experimental
using Career_Check_TopLevelUnlocked_t = uint64_t(void *scene, uint32_t level_id, void *out);
inline constexpr fn<"Career_Check_TopLevelUnlocked", Career_Check_TopLevelUnlocked_t> Career_Check_TopLevelUnlocked{}; // L2 experimental
inline constexpr address<"Career_Collect_PlayerTeamDriverContracts"> Career_Collect_PlayerTeamDriverContracts{}; // L2 experimental
inline constexpr address<"Career_Collect_TeamUpgradeEntries"> Career_Collect_TeamUpgradeEntries{}; // L2 experimental
using Career_Collect_UpgradeEventsForDay_t = void *(void *upgrades_comp, void *out);
inline constexpr fn<"Career_Collect_UpgradeEventsForDay", Career_Collect_UpgradeEventsForDay_t> Career_Collect_UpgradeEventsForDay{}; // L2 experimental
using Career_Complete_PerkTreeNode_t = uint8_t(void *node);
inline constexpr fn<"Career_Complete_PerkTreeNode", Career_Complete_PerkTreeNode_t> Career_Complete_PerkTreeNode{}; // L2 experimental
inline constexpr address<"Career_Compute_InstalledUpgradePerformance"> Career_Compute_InstalledUpgradePerformance{}; // L2 experimental
inline constexpr address<"Career_Compute_InstalledUpgradePerformanceByType"> Career_Compute_InstalledUpgradePerformanceByType{}; // L2 experimental
using Career_Condition_CopyParams_Ctor_t = void *(void *self, void *owner, void *ctx, void *params_vec);
inline constexpr fn<"Career_Condition_CopyParams_Ctor", Career_Condition_CopyParams_Ctor_t> Career_Condition_CopyParams_Ctor{}; // L2 experimental
inline constexpr address<"Career_Condition_FacilityUpgradeType_Ctor"> Career_Condition_FacilityUpgradeType_Ctor{}; // L2 experimental
inline constexpr address<"Career_Condition_FacilityUpgradedWithinDays_Ctor"> Career_Condition_FacilityUpgradedWithinDays_Ctor{}; // L2 experimental
inline constexpr address<"Career_Confirm_FacilityUpgradePurchase"> Career_Confirm_FacilityUpgradePurchase{}; // L2 experimental
using Career_Construct_RnDDeptCard_t = void *(void *card, void *ctx, void *builder, void *allocator, const char *name, const char *icon, uint8_t dept);
inline constexpr fn<"Career_Construct_RnDDeptCard", Career_Construct_RnDDeptCard_t> Career_Construct_RnDDeptCard{}; // L2 experimental
inline constexpr address<"Career_ContractHubVM_OnActivate"> Career_ContractHubVM_OnActivate{}; // L2 experimental
using Career_ContractIntro_OnEnter_t = uint64_t(void *state);
inline constexpr fn<"Career_ContractIntro_OnEnter", Career_ContractIntro_OnEnter_t> Career_ContractIntro_OnEnter{}; // L2 experimental
using Career_ContractIntro_OnUpdate_t = uint32_t *(void *state, uint32_t *link);
inline constexpr fn<"Career_ContractIntro_OnUpdate", Career_ContractIntro_OnUpdate_t> Career_ContractIntro_OnUpdate{}; // L2 experimental
using Career_ContractNegotiationsHostVM_OnActivate_t = void(void *vm);
inline constexpr fn<"Career_ContractNegotiationsHostVM_OnActivate", Career_ContractNegotiationsHostVM_OnActivate_t> Career_ContractNegotiationsHostVM_OnActivate{}; // L2 experimental
using Career_ContractNegotiationsHostVM_OnDeactivate_t = void(void *vm);
inline constexpr fn<"Career_ContractNegotiationsHostVM_OnDeactivate", Career_ContractNegotiationsHostVM_OnDeactivate_t> Career_ContractNegotiationsHostVM_OnDeactivate{}; // L2 experimental
using Career_ContractNegotiationsHost_RegisterMessageHandlers_Alloc_t = void(void *host);
inline constexpr fn<"Career_ContractNegotiationsHost_RegisterMessageHandlers_Alloc", Career_ContractNegotiationsHost_RegisterMessageHandlers_Alloc_t> Career_ContractNegotiationsHost_RegisterMessageHandlers_Alloc{}; // L2 experimental
using Career_ContractNegotiationsView_BindStatRows_t = void(void *view);
inline constexpr fn<"Career_ContractNegotiationsView_BindStatRows", Career_ContractNegotiationsView_BindStatRows_t> Career_ContractNegotiationsView_BindStatRows{}; // L2 experimental
using Career_ContractOutro_ClearFlowHandlers_t = void(void *state);
inline constexpr fn<"Career_ContractOutro_ClearFlowHandlers", Career_ContractOutro_ClearFlowHandlers_t> Career_ContractOutro_ClearFlowHandlers{}; // L2 experimental
using Career_ContractOutro_Destruct_t = void **(void **self, int free_flag);
inline constexpr fn<"Career_ContractOutro_Destruct", Career_ContractOutro_Destruct_t> Career_ContractOutro_Destruct{}; // L2 experimental
using Career_ContractOutro_OnEnter_t = uint64_t(void *state);
inline constexpr fn<"Career_ContractOutro_OnEnter", Career_ContractOutro_OnEnter_t> Career_ContractOutro_OnEnter{}; // L2 experimental
using Career_ContractOutro_OnExit_t = uint64_t(void *state);
inline constexpr fn<"Career_ContractOutro_OnExit", Career_ContractOutro_OnExit_t> Career_ContractOutro_OnExit{}; // L2 experimental
using Career_ContractOutro_OnUpdate_t = uint32_t *(void *state, uint32_t *link);
inline constexpr fn<"Career_ContractOutro_OnUpdate", Career_ContractOutro_OnUpdate_t> Career_ContractOutro_OnUpdate{}; // L2 experimental
inline constexpr address<"Career_ContractPerksVM_OnActivate"> Career_ContractPerksVM_OnActivate{}; // L2 experimental
using Career_ContractRenewal_AiApproach_t = void(void *sys, const uint32_t *driver_key);
inline constexpr fn<"Career_ContractRenewal_AiApproach", Career_ContractRenewal_AiApproach_t> Career_ContractRenewal_AiApproach{}; // L2 experimental
using Career_ContractRenewal_AllSlotsMeetMinimum_t = uint64_t(void *comp);
inline constexpr fn<"Career_ContractRenewal_AllSlotsMeetMinimum", Career_ContractRenewal_AllSlotsMeetMinimum_t> Career_ContractRenewal_AllSlotsMeetMinimum{}; // L2 experimental
inline constexpr address<"Career_ContractRenewal_Deserialise"> Career_ContractRenewal_Deserialise{}; // L2 experimental
using Career_ContractRenewal_PostAiApproachMessage_t = void(void *comp, uint32_t event_key);
inline constexpr fn<"Career_ContractRenewal_PostAiApproachMessage", Career_ContractRenewal_PostAiApproachMessage_t> Career_ContractRenewal_PostAiApproachMessage{}; // L2 experimental
using Career_ContractRenewal_PushUniqueDriverKey_t = void(void *list, uint32_t *key);
inline constexpr fn<"Career_ContractRenewal_PushUniqueDriverKey", Career_ContractRenewal_PushUniqueDriverKey_t> Career_ContractRenewal_PushUniqueDriverKey{}; // L2 experimental
inline constexpr address<"Career_ContractRenewal_Serialise"> Career_ContractRenewal_Serialise{}; // L2 experimental
using Career_ContractUpdate_OnEnter_t = uint64_t(void *state);
inline constexpr fn<"Career_ContractUpdate_OnEnter", Career_ContractUpdate_OnEnter_t> Career_ContractUpdate_OnEnter{}; // L2 experimental
using Career_ContractUpdate_OnExit_t = uint64_t(void *state);
inline constexpr fn<"Career_ContractUpdate_OnExit", Career_ContractUpdate_OnExit_t> Career_ContractUpdate_OnExit{}; // L2 experimental
using Career_ContractUpdate_OnUpdate_t = uint32_t *(void *state, uint32_t *link);
inline constexpr fn<"Career_ContractUpdate_OnUpdate", Career_ContractUpdate_OnUpdate_t> Career_ContractUpdate_OnUpdate{}; // L2 experimental
using Career_ContractUpdate_SetFlowHandlers_t = void(void *state, char enable);
inline constexpr fn<"Career_ContractUpdate_SetFlowHandlers", Career_ContractUpdate_SetFlowHandlers_t> Career_ContractUpdate_SetFlowHandlers{}; // L2 experimental
inline constexpr address<"Career_Contract_PostCompensationTransactions"> Career_Contract_PostCompensationTransactions{}; // L2 experimental
using Career_Copy_SponsorSlotRecord_t = void *(void *dst, int64_t src);
inline constexpr fn<"Career_Copy_SponsorSlotRecord", Career_Copy_SponsorSlotRecord_t> Career_Copy_SponsorSlotRecord{}; // L2 experimental
inline constexpr address<"Career_CostCap_LoadConfig"> Career_CostCap_LoadConfig{}; // L2 experimental
inline constexpr address<"Career_CostCap_ReadSaveData"> Career_CostCap_ReadSaveData{}; // L2 experimental
inline constexpr address<"Career_CostCap_WriteSaveData"> Career_CostCap_WriteSaveData{}; // L2 experimental
inline constexpr address<"Career_Create_ResourcePointsContext"> Career_Create_ResourcePointsContext{}; // L2 experimental
using Career_Create_SeasonWeekend_t = void *(void *season, uint64_t index, uint8_t track, const char *name);
inline constexpr fn<"Career_Create_SeasonWeekend", Career_Create_SeasonWeekend_t> Career_Create_SeasonWeekend{}; // L2 experimental
inline constexpr address<"Career_DamageBudgetRefund_ReadSaveData"> Career_DamageBudgetRefund_ReadSaveData{}; // L2 experimental
inline constexpr address<"Career_DamageBudgetRefund_WriteSaveData"> Career_DamageBudgetRefund_WriteSaveData{}; // L2 experimental
using Career_Decide_ContractNegotiation_t = uint32_t *(void *state, uint32_t *link);
inline constexpr fn<"Career_Decide_ContractNegotiation", Career_Decide_ContractNegotiation_t> Career_Decide_ContractNegotiation{}; // L2 experimental
inline constexpr address<"Career_DeptEventReward_CreateUpgradeDiscount"> Career_DeptEventReward_CreateUpgradeDiscount{}; // L2 experimental
inline constexpr address<"Career_DeptEventReward_FormatDescription"> Career_DeptEventReward_FormatDescription{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_CheckStoredUpgradeForDriver"> Career_DeptEvent_CheckStoredUpgradeForDriver{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_GetDriverMagicOffset"> Career_DeptEvent_GetDriverMagicOffset{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_GetSpawnRate"> Career_DeptEvent_GetSpawnRate{}; // L2 experimental
using Career_DeptEvent_LookupOrAllocate_t = void *(void *mgr, uint8_t *key, char create);
inline constexpr fn<"Career_DeptEvent_LookupOrAllocate", Career_DeptEvent_LookupOrAllocate_t> Career_DeptEvent_LookupOrAllocate{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_ParseDepartment"> Career_DeptEvent_ParseDepartment{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_ReadConfigEntry"> Career_DeptEvent_ReadConfigEntry{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_ReadRandomParameters"> Career_DeptEvent_ReadRandomParameters{}; // L2 experimental
inline constexpr address<"Career_DeptEvent_RegisterMessageHandlersA"> Career_DeptEvent_RegisterMessageHandlersA{}; // L2 experimental
using Career_DeptEvent_SimulateOutcomeInsert_t = int64_t(void *mgr, uint32_t *key, int count);
inline constexpr fn<"Career_DeptEvent_SimulateOutcomeInsert", Career_DeptEvent_SimulateOutcomeInsert_t> Career_DeptEvent_SimulateOutcomeInsert{}; // L2 experimental
using Career_Deserialise_ClaimedFlag_t = uint32_t(void *obj, void *json_value);
inline constexpr fn<"Career_Deserialise_ClaimedFlag", Career_Deserialise_ClaimedFlag_t> Career_Deserialise_ClaimedFlag{}; // L2 experimental
using Career_Despawn_UpgradeList_A_t = void(void *self);
inline constexpr fn<"Career_Despawn_UpgradeList_A", Career_Despawn_UpgradeList_A_t> Career_Despawn_UpgradeList_A{}; // L2 experimental
inline constexpr address<"Career_DevelopmentPerksVM_BuildAcclaim"> Career_DevelopmentPerksVM_BuildAcclaim{}; // L2 experimental
using Career_DriverAcademies_MapFindOrAdd_t = void(void *map, DriverAcademyMapSlot *slot, const uint16_t *key);
inline constexpr fn<"Career_DriverAcademies_MapFindOrAdd", Career_DriverAcademies_MapFindOrAdd_t> Career_DriverAcademies_MapFindOrAdd{}; // L2 experimental
using Career_DriverContractEvent_Destroy_t = void(void **ev);
inline constexpr fn<"Career_DriverContractEvent_Destroy", Career_DriverContractEvent_Destroy_t> Career_DriverContractEvent_Destroy{}; // L2 experimental
inline constexpr address<"Career_DriverContractsMyTeam_AddPendingEntry"> Career_DriverContractsMyTeam_AddPendingEntry{}; // L2 experimental
inline constexpr address<"Career_DriverContractsMyTeam_PostApproachMessages"> Career_DriverContractsMyTeam_PostApproachMessages{}; // L2 experimental
inline constexpr address<"Career_DriverContractsMyTeam_RemovePendingByState"> Career_DriverContractsMyTeam_RemovePendingByState{}; // L2 experimental
inline constexpr address<"Career_DriverContractsMyTeam_SubmitOffer"> Career_DriverContractsMyTeam_SubmitOffer{}; // L2 experimental
inline constexpr address<"Career_DriverContracts_BuildDriverContractQuote"> Career_DriverContracts_BuildDriverContractQuote{}; // L2 experimental
inline constexpr address<"Career_DriverContracts_HasTeamForDriver"> Career_DriverContracts_HasTeamForDriver{}; // L2 experimental
using Career_DriverNumbers_CheckExisting_t = void(void *car_numbers, uint32_t driver);
inline constexpr fn<"Career_DriverNumbers_CheckExisting", Career_DriverNumbers_CheckExisting_t> Career_DriverNumbers_CheckExisting{}; // L2 experimental
inline constexpr address<"Career_Enter_CorporateTopLevel"> Career_Enter_CorporateTopLevel{}; // L2 experimental
inline constexpr address<"Career_Enter_EngineeringTopLevel"> Career_Enter_EngineeringTopLevel{}; // L2 experimental
inline constexpr address<"Career_Enter_FacilityUpgrades"> Career_Enter_FacilityUpgrades{}; // L2 experimental
inline constexpr address<"Career_Enter_PersonnelTopLevel"> Career_Enter_PersonnelTopLevel{}; // L2 experimental
using Career_Eval_Curve_t = float(void *curve, float x);
inline constexpr fn<"Career_Eval_Curve", Career_Eval_Curve_t> Career_Eval_Curve{}; // L2 experimental
using Career_Eval_FacilityNodeEffectCurve_t = int(void *node);
inline constexpr fn<"Career_Eval_FacilityNodeEffectCurve", Career_Eval_FacilityNodeEffectCurve_t> Career_Eval_FacilityNodeEffectCurve{}; // L2 experimental
using Career_Exit_RnDToCareerHub_t = void(void *self, uint32_t a2);
inline constexpr fn<"Career_Exit_RnDToCareerHub", Career_Exit_RnDToCareerHub_t> Career_Exit_RnDToCareerHub{}; // L2 experimental
inline constexpr address<"Career_Facilities_ReadSaveData"> Career_Facilities_ReadSaveData{}; // L2 experimental
inline constexpr address<"Career_Facilities_WriteSaveData"> Career_Facilities_WriteSaveData{}; // L2 experimental
inline constexpr address<"Career_Find_CareerSaveByPredicate"> Career_Find_CareerSaveByPredicate{}; // L2 experimental
inline constexpr address<"Career_Find_EntryByStatKey_388"> Career_Find_EntryByStatKey_388{}; // L2 experimental
inline constexpr address<"Career_Find_FacilityTopLevelByKey"> Career_Find_FacilityTopLevelByKey{}; // L2 experimental
using Career_Find_RenownLevelEntryForTeam_t = void *(void *comp, void *out, uint8_t *team_key);
inline constexpr fn<"Career_Find_RenownLevelEntryForTeam", Career_Find_RenownLevelEntryForTeam_t> Career_Find_RenownLevelEntryForTeam{}; // L2 experimental
inline constexpr address<"Career_Find_SlotIndexByKey"> Career_Find_SlotIndexByKey{}; // L2 experimental
using Career_Find_TeamStatRange_t = void(void *out, void *team_stats, uint32_t team_key, void *stat_name);
inline constexpr fn<"Career_Find_TeamStatRange", Career_Find_TeamStatRange_t> Career_Find_TeamStatRange{}; // L2 experimental
inline constexpr address<"Career_Find_UpgradeEntryByKey"> Career_Find_UpgradeEntryByKey{}; // L2 experimental
inline constexpr address<"Career_Format_FacilityPhaseCapacityText"> Career_Format_FacilityPhaseCapacityText{}; // L2 experimental
using Career_Format_MoraleDepartmentEffectText_t = void(void *effect, void *out_text);
inline constexpr fn<"Career_Format_MoraleDepartmentEffectText", Career_Format_MoraleDepartmentEffectText_t> Career_Format_MoraleDepartmentEffectText{}; // L2 experimental
using Career_Format_RnDDaysRemaining_t = uint64_t *(int64_t *self, uint64_t *a2, int a3);
inline constexpr fn<"Career_Format_RnDDaysRemaining", Career_Format_RnDDaysRemaining_t> Career_Format_RnDDaysRemaining{}; // L2 experimental
inline constexpr address<"Career_GameValue_GetBankBalance"> Career_GameValue_GetBankBalance{}; // L2 experimental
inline constexpr address<"Career_GameValue_GetNetWeeklyIncome"> Career_GameValue_GetNetWeeklyIncome{}; // L2 experimental
inline constexpr address<"Career_GameValue_GetUpgradesFailedDuringTimeAdvance"> Career_GameValue_GetUpgradesFailedDuringTimeAdvance{}; // L2 experimental
inline constexpr address<"Career_GameValue_GetUpgradesResearchedDuringTimeAdvance"> Career_GameValue_GetUpgradesResearchedDuringTimeAdvance{}; // L2 experimental
inline constexpr address<"Career_Get_ActivityRecordByIndex"> Career_Get_ActivityRecordByIndex{}; // L2 experimental
using Career_Get_BackgroundOutcomeField_t = uint32_t(void *row_name);
inline constexpr fn<"Career_Get_BackgroundOutcomeField", Career_Get_BackgroundOutcomeField_t> Career_Get_BackgroundOutcomeField{}; // L2 experimental
using Career_Get_CareerLengthDays_Thunk_t = uint32_t(void *time);
inline constexpr fn<"Career_Get_CareerLengthDays_Thunk", Career_Get_CareerLengthDays_Thunk_t> Career_Get_CareerLengthDays_Thunk{}; // L2 experimental
inline constexpr address<"Career_Get_ContractSlotLocText"> Career_Get_ContractSlotLocText{}; // L2 experimental
inline constexpr address<"Career_Get_ContractSlotTypeName"> Career_Get_ContractSlotTypeName{}; // L2 experimental
inline constexpr address<"Career_Get_DriverContractRecordByKey"> Career_Get_DriverContractRecordByKey{}; // L2 experimental
using Career_Get_DriverOverallRating_t = float(void *self, void *data, const uint32_t *driver, bool round);
inline constexpr fn<"Career_Get_DriverOverallRating", Career_Get_DriverOverallRating_t> Career_Get_DriverOverallRating{}; // L2 experimental, game::championship::DriverStatisticsComponent
inline constexpr address<"Career_Get_FacilityNodeList"> Career_Get_FacilityNodeList{}; // L2 experimental
inline constexpr address<"Career_Get_FacilityObjectHandleById"> Career_Get_FacilityObjectHandleById{}; // L2 experimental
using Career_Get_FacilityUpgradeSlotEntry_t = void *(void *scene, int index);
inline constexpr fn<"Career_Get_FacilityUpgradeSlotEntry", Career_Get_FacilityUpgradeSlotEntry_t> Career_Get_FacilityUpgradeSlotEntry{}; // L2 experimental
inline constexpr address<"Career_Get_OwnerPerkByIndex"> Career_Get_OwnerPerkByIndex{}; // L2 experimental
inline constexpr address<"Career_Get_RenownLevelInfo"> Career_Get_RenownLevelInfo{}; // L2 experimental
using Career_Get_RnDPurchaseBlockReason_t = uint64_t(void *self, uint32_t a2);
inline constexpr fn<"Career_Get_RnDPurchaseBlockReason", Career_Get_RnDPurchaseBlockReason_t> Career_Get_RnDPurchaseBlockReason{}; // L2 experimental
using Career_Get_RnDRecognitionDescription_t = uint64_t(void *self, void *a2, int64_t *a3);
inline constexpr fn<"Career_Get_RnDRecognitionDescription", Career_Get_RnDRecognitionDescription_t> Career_Get_RnDRecognitionDescription{}; // L2 experimental
using Career_Get_RnDTeamTypeIcon_t = int64_t *(int64_t *self, char a2, void *a3);
inline constexpr fn<"Career_Get_RnDTeamTypeIcon", Career_Get_RnDTeamTypeIcon_t> Career_Get_RnDTeamTypeIcon{}; // L2 experimental
using Career_Get_RpWallet_t = void(void *wallets, uint64_t *out, uint32_t driver_key);
inline constexpr fn<"Career_Get_RpWallet", Career_Get_RpWallet_t> Career_Get_RpWallet{}; // L2 experimental
inline constexpr address<"Career_Get_SponsorIconOverrideSet"> Career_Get_SponsorIconOverrideSet{}; // L2 experimental
inline constexpr address<"Career_Get_TeamBankBudgetCategoryValue"> Career_Get_TeamBankBudgetCategoryValue{}; // L2 experimental
using Career_Get_TeamRenownData_t = void *(void *rf, void *out_weak, uint32_t team_key);
inline constexpr fn<"Career_Get_TeamRenownData", Career_Get_TeamRenownData_t> Career_Get_TeamRenownData{}; // L2 experimental
using Career_Get_TeamStatClampedValue_t = void *(float *out, void *team_comp, uint32_t team_key, void *stat_name);
inline constexpr fn<"Career_Get_TeamStatClampedValue", Career_Get_TeamStatClampedValue_t> Career_Get_TeamStatClampedValue{}; // L2 experimental
inline constexpr address<"Career_Get_UpgradeCostAdjust"> Career_Get_UpgradeCostAdjust{}; // L2 experimental
using Career_Handle_HideRnDPanel_t = uint64_t(void *self, void *a2);
inline constexpr fn<"Career_Handle_HideRnDPanel", Career_Handle_HideRnDPanel_t> Career_Handle_HideRnDPanel{}; // L2 experimental
using Career_Handle_RnDBackFromDepartments_A_t = uint64_t(void *self);
inline constexpr fn<"Career_Handle_RnDBackFromDepartments_A", Career_Handle_RnDBackFromDepartments_A_t> Career_Handle_RnDBackFromDepartments_A{}; // L2 experimental
using Career_Handle_RnDBackFromDepartments_B_t = uint64_t(void *self);
inline constexpr fn<"Career_Handle_RnDBackFromDepartments_B", Career_Handle_RnDBackFromDepartments_B_t> Career_Handle_RnDBackFromDepartments_B{}; // L2 experimental
using Career_Handle_RnDChallengeAppliedMessage_t = void(int64_t *self);
inline constexpr fn<"Career_Handle_RnDChallengeAppliedMessage", Career_Handle_RnDChallengeAppliedMessage_t> Career_Handle_RnDChallengeAppliedMessage{}; // L2 experimental
inline constexpr address<"Career_HubCamera_UpdateRnd"> Career_HubCamera_UpdateRnd{}; // L2 experimental
inline constexpr address<"Career_Init_ConditionTypeRegistry"> Career_Init_ConditionTypeRegistry{}; // L2 experimental
inline constexpr address<"Career_Init_ContractPreferenceTables"> Career_Init_ContractPreferenceTables{}; // L2 experimental
inline constexpr address<"Career_Init_MyTeamRewardTypeRegistry"> Career_Init_MyTeamRewardTypeRegistry{}; // L2 experimental
inline constexpr address<"Career_Init_RnDUpgradeRewardPanel"> Career_Init_RnDUpgradeRewardPanel{}; // L2 experimental
inline constexpr address<"Career_Init_SponsorTypeEntry"> Career_Init_SponsorTypeEntry{}; // L2 experimental
inline constexpr address<"Career_Init_UpgradesWorkforceStatRow"> Career_Init_UpgradesWorkforceStatRow{}; // L2 experimental
inline constexpr address<"Career_InvitationalAcclaimReward_BuildText"> Career_InvitationalAcclaimReward_BuildText{}; // L2 experimental
using Career_Is_RnDPlayerControlled_t = uint64_t(int64_t *self, uint64_t a2);
inline constexpr fn<"Career_Is_RnDPlayerControlled", Career_Is_RnDPlayerControlled_t> Career_Is_RnDPlayerControlled{}; // L2 experimental
inline constexpr address<"Career_Load_DepartmentEventOutcomes"> Career_Load_DepartmentEventOutcomes{}; // L2 experimental
inline constexpr address<"Career_Load_DeptEventState"> Career_Load_DeptEventState{}; // L2 experimental
using Career_Load_EngineSupplierUpgrades_t = void(void *self);
inline constexpr fn<"Career_Load_EngineSupplierUpgrades", Career_Load_EngineSupplierUpgrades_t> Career_Load_EngineSupplierUpgrades{}; // L2 experimental
inline constexpr address<"Career_Load_FacilitiesStageScene"> Career_Load_FacilitiesStageScene{}; // L2 experimental
inline constexpr address<"Career_Load_FacilitiesStageScene_2"> Career_Load_FacilitiesStageScene_2{}; // L2 experimental
inline constexpr address<"Career_Load_MyTeamRewardDefinitions"> Career_Load_MyTeamRewardDefinitions{}; // L2 experimental
inline constexpr address<"Career_Load_MyTeamStatRenown"> Career_Load_MyTeamStatRenown{}; // L2 experimental
using Career_Load_PendingDriverContract_t = int(void *contract, void *save_node, uint64_t version);
inline constexpr fn<"Career_Load_PendingDriverContract", Career_Load_PendingDriverContract_t> Career_Load_PendingDriverContract{}; // L2 experimental
inline constexpr address<"Career_Load_RndChallengeState"> Career_Load_RndChallengeState{}; // L2 experimental
inline constexpr address<"Career_Load_RndHeads"> Career_Load_RndHeads{}; // L2 experimental
inline constexpr address<"Career_Load_SponsorDecalIncomeTiers"> Career_Load_SponsorDecalIncomeTiers{}; // L2 experimental
inline constexpr address<"Career_Load_UpgradeCounts"> Career_Load_UpgradeCounts{}; // L2 experimental
using Career_Load_UpgradeTreeWaypointPath_t = void(void *self, uint64_t a2);
inline constexpr fn<"Career_Load_UpgradeTreeWaypointPath", Career_Load_UpgradeTreeWaypointPath_t> Career_Load_UpgradeTreeWaypointPath{}; // L2 experimental
using Career_Load_UpgradeTypeMetadata_t = void(void *self, uint64_t a2);
inline constexpr fn<"Career_Load_UpgradeTypeMetadata", Career_Load_UpgradeTypeMetadata_t> Career_Load_UpgradeTypeMetadata{}; // L2 experimental
using Career_Msg_OnBankAccountFundsMessage_t = uint64_t(void *owner, void *msg);
inline constexpr fn<"Career_Msg_OnBankAccountFundsMessage", Career_Msg_OnBankAccountFundsMessage_t> Career_Msg_OnBankAccountFundsMessage{}; // L2 experimental
inline constexpr address<"Career_Msg_SubscribeBankAccountValidTransaction_A"> Career_Msg_SubscribeBankAccountValidTransaction_A{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_BindSelectedActivity"> Career_MyTeamActivities_BindSelectedActivity{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_BuildActivityPanel"> Career_MyTeamActivities_BuildActivityPanel{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_BuildSlotGrid"> Career_MyTeamActivities_BuildSlotGrid{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_ExitWithoutPick"> Career_MyTeamActivities_ExitWithoutPick{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_FindGroupByFlatIndex"> Career_MyTeamActivities_FindGroupByFlatIndex{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_GetSelectedSlotRecord"> Career_MyTeamActivities_GetSelectedSlotRecord{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_MenuMain"> Career_MyTeamActivities_MenuMain{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_OnSelect"> Career_MyTeamActivities_OnSelect{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_Refresh"> Career_MyTeamActivities_Refresh{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_RefreshDetailPanel"> Career_MyTeamActivities_RefreshDetailPanel{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_SetEditMode"> Career_MyTeamActivities_SetEditMode{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_SetPanelFlagNodes"> Career_MyTeamActivities_SetPanelFlagNodes{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_UpdateSlot"> Career_MyTeamActivities_UpdateSlot{}; // L2 experimental
inline constexpr address<"Career_MyTeamActivities_UpdateSlotButtons"> Career_MyTeamActivities_UpdateSlotButtons{}; // L2 experimental
inline constexpr address<"Career_MyTeamCareerConfirm_SetupTutorial"> Career_MyTeamCareerConfirm_SetupTutorial{}; // L2 experimental
using Career_MyTeamContract_OnDriverSelected_t = void(void *screen, void **event);
inline constexpr fn<"Career_MyTeamContract_OnDriverSelected", Career_MyTeamContract_OnDriverSelected_t> Career_MyTeamContract_OnDriverSelected{}; // L2 experimental
using Career_MyTeamContract_OnSendContractPressed_t = void(void *screen, void **event);
inline constexpr fn<"Career_MyTeamContract_OnSendContractPressed", Career_MyTeamContract_OnSendContractPressed_t> Career_MyTeamContract_OnSendContractPressed{}; // L2 experimental
using Career_MyTeamContract_OnSendCounterOffer_t = uint64_t(void *screen, void **event, void *arg);
inline constexpr fn<"Career_MyTeamContract_OnSendCounterOffer", Career_MyTeamContract_OnSendCounterOffer_t> Career_MyTeamContract_OnSendCounterOffer{}; // L2 experimental
using Career_MyTeamContract_OnStopTalksPressed_t = void(void *screen, void **event);
inline constexpr fn<"Career_MyTeamContract_OnStopTalksPressed", Career_MyTeamContract_OnStopTalksPressed_t> Career_MyTeamContract_OnStopTalksPressed{}; // L2 experimental
inline constexpr address<"Career_MyTeamCorporateSetupVM_BuildCards"> Career_MyTeamCorporateSetupVM_BuildCards{}; // L2 experimental
inline constexpr address<"Career_MyTeamCorporateSetupVM_OnAction"> Career_MyTeamCorporateSetupVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_MyTeamCorporateSetupVM_OnActivate"> Career_MyTeamCorporateSetupVM_OnActivate{}; // L2 experimental
inline constexpr address<"Career_MyTeamCustomisationOnboardingIncomplete_Check"> Career_MyTeamCustomisationOnboardingIncomplete_Check{}; // L2 experimental
using Career_MyTeamDriverComms_BuildScreen_t = uint64_t(void *screen);
inline constexpr fn<"Career_MyTeamDriverComms_BuildScreen", Career_MyTeamDriverComms_BuildScreen_t> Career_MyTeamDriverComms_BuildScreen{}; // L2 experimental
inline constexpr address<"Career_MyTeamDriverComms_RefreshDriverCards"> Career_MyTeamDriverComms_RefreshDriverCards{}; // L2 experimental
using Career_MyTeamDriverComms_UpdateDriverCard_t = void(void *screen, void *driver_slot, void *item);
inline constexpr fn<"Career_MyTeamDriverComms_UpdateDriverCard", Career_MyTeamDriverComms_UpdateDriverCard_t> Career_MyTeamDriverComms_UpdateDriverCard{}; // L2 experimental
using Career_MyTeamDriverDetails_BuildScreen_t = uint64_t(void *screen);
inline constexpr fn<"Career_MyTeamDriverDetails_BuildScreen", Career_MyTeamDriverDetails_BuildScreen_t> Career_MyTeamDriverDetails_BuildScreen{}; // L2 experimental
inline constexpr address<"Career_MyTeamDriverDetails_RefreshOffer"> Career_MyTeamDriverDetails_RefreshOffer{}; // L2 experimental
inline constexpr address<"Career_MyTeamF1Length_ChoiceTable"> Career_MyTeamF1Length_ChoiceTable{}; // L2 experimental
inline constexpr address<"Career_MyTeamInitialEntryActivities_Event"> Career_MyTeamInitialEntryActivities_Event{}; // L2 experimental
inline constexpr address<"Career_MyTeamLeftWithoutResearch_Check"> Career_MyTeamLeftWithoutResearch_Check{}; // L2 experimental
inline constexpr address<"Career_MyTeamManagement_Events"> Career_MyTeamManagement_Events{}; // L2 experimental
inline constexpr address<"Career_MyTeamOffice_BuildCards"> Career_MyTeamOffice_BuildCards{}; // L2 experimental
inline constexpr address<"Career_MyTeamOwnerBackgroundVM_OnAction"> Career_MyTeamOwnerBackgroundVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_MyTeamRenownReward_Describe"> Career_MyTeamRenownReward_Describe{}; // L2 experimental
inline constexpr address<"Career_MyTeamResearchProjectStarted_Event"> Career_MyTeamResearchProjectStarted_Event{}; // L2 experimental
inline constexpr address<"Career_MyTeamRewards_GetRewardName"> Career_MyTeamRewards_GetRewardName{}; // L2 experimental
inline constexpr address<"Career_MyTeamSeasonal_SetStage"> Career_MyTeamSeasonal_SetStage{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetupRecord_InitDefault"> Career_MyTeamSetupRecord_InitDefault{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetupVM_FocusSelection"> Career_MyTeamSetupVM_FocusSelection{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetupVM_RefreshSelection"> Career_MyTeamSetupVM_RefreshSelection{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetupVM_ResetStepSelection"> Career_MyTeamSetupVM_ResetStepSelection{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetupVM_SetStep"> Career_MyTeamSetupVM_SetStep{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetupVM_UpdateCostIcon"> Career_MyTeamSetupVM_UpdateCostIcon{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetup_AdvanceFadeOut"> Career_MyTeamSetup_AdvanceFadeOut{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetup_CommitTeamIdentity"> Career_MyTeamSetup_CommitTeamIdentity{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetup_CopyOptionIdList"> Career_MyTeamSetup_CopyOptionIdList{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetup_IsSelectionComplete"> Career_MyTeamSetup_IsSelectionComplete{}; // L2 experimental
inline constexpr address<"Career_MyTeamSetup_StageSet"> Career_MyTeamSetup_StageSet{}; // L2 experimental
inline constexpr address<"Career_MyTeamSponsorDiscoveryReward_Describe"> Career_MyTeamSponsorDiscoveryReward_Describe{}; // L2 experimental
inline constexpr address<"Career_MyTeamSponsorUnlockReward_Describe"> Career_MyTeamSponsorUnlockReward_Describe{}; // L2 experimental
inline constexpr address<"Career_MyTeamStageTags_SetFromState"> Career_MyTeamStageTags_SetFromState{}; // L2 experimental
inline constexpr address<"Career_MyTeamTopLevels_Exit"> Career_MyTeamTopLevels_Exit{}; // L2 experimental
inline constexpr address<"Career_MyTeamVM_RefreshSponsorSelection"> Career_MyTeamVM_RefreshSponsorSelection{}; // L2 experimental
inline constexpr address<"Career_MyTeamVM_RequestThumbnails"> Career_MyTeamVM_RequestThumbnails{}; // L2 experimental
inline constexpr address<"Career_MyTeamYoungDriverProgramme_SelectDriver"> Career_MyTeamYoungDriverProgramme_SelectDriver{}; // L2 experimental
inline constexpr address<"Career_MyTeam_ReadSaveSections"> Career_MyTeam_ReadSaveSections{}; // L2 experimental
using Career_Needs_DriverContract_t = bool(void *contracts);
inline constexpr fn<"Career_Needs_DriverContract", Career_Needs_DriverContract_t> Career_Needs_DriverContract{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiationsB_EndFlow"> Career_OnlineNegotiationsB_EndFlow{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiationsB_OnNegotiationsFinished"> Career_OnlineNegotiationsB_OnNegotiationsFinished{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiationsB_ProcessContractRecv"> Career_OnlineNegotiationsB_ProcessContractRecv{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiationsB_ProcessContractSent"> Career_OnlineNegotiationsB_ProcessContractSent{}; // L2 experimental
using Career_OnlineNegotiations_CheckPlayerTurnGate_t = uint64_t(void *neg);
inline constexpr fn<"Career_OnlineNegotiations_CheckPlayerTurnGate", Career_OnlineNegotiations_CheckPlayerTurnGate_t> Career_OnlineNegotiations_CheckPlayerTurnGate{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_EndFlow"> Career_OnlineNegotiations_EndFlow{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_IsCandidateOnContractedTeam"> Career_OnlineNegotiations_IsCandidateOnContractedTeam{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_IsLeadPlayerParticipating"> Career_OnlineNegotiations_IsLeadPlayerParticipating{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_IsLeadPlayersTurnTeamSel"> Career_OnlineNegotiations_IsLeadPlayersTurnTeamSel{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_LoadEventFromState"> Career_OnlineNegotiations_LoadEventFromState{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_OnNegotiationsFinished"> Career_OnlineNegotiations_OnNegotiationsFinished{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_ProcessContractRecv"> Career_OnlineNegotiations_ProcessContractRecv{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_ProcessContractSent"> Career_OnlineNegotiations_ProcessContractSent{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_PushEvent12"> Career_OnlineNegotiations_PushEvent12{}; // L2 experimental
using Career_OnlineNegotiations_RegisterMessageHandlers_Alloc_t = void(void *neg);
inline constexpr fn<"Career_OnlineNegotiations_RegisterMessageHandlers_Alloc", Career_OnlineNegotiations_RegisterMessageHandlers_Alloc_t> Career_OnlineNegotiations_RegisterMessageHandlers_Alloc{}; // L2 experimental
inline constexpr address<"Career_OnlineNegotiations_ShouldShowFiredState"> Career_OnlineNegotiations_ShouldShowFiredState{}; // L2 experimental
using Career_OnlineNegotiations_SortCandidatesByState_t = void *(void *neg);
inline constexpr fn<"Career_OnlineNegotiations_SortCandidatesByState", Career_OnlineNegotiations_SortCandidatesByState_t> Career_OnlineNegotiations_SortCandidatesByState{}; // L2 experimental
using Career_Open_RnDDevelopmentConfirmation_t = void(void *self);
inline constexpr fn<"Career_Open_RnDDevelopmentConfirmation", Career_Open_RnDDevelopmentConfirmation_t> Career_Open_RnDDevelopmentConfirmation{}; // L2 experimental
inline constexpr address<"Career_OwnerSetup_SumSelectedPerkValues"> Career_OwnerSetup_SumSelectedPerkValues{}; // L2 experimental
inline constexpr address<"Career_OxpNotificationVM_RefreshSponsorLoyalty"> Career_OxpNotificationVM_RefreshSponsorLoyalty{}; // L2 experimental
using Career_Parse_UpgradeDepartmentMoraleEntry_t = void(void **out, void *allocator, void *unused, void *xml_node);
inline constexpr fn<"Career_Parse_UpgradeDepartmentMoraleEntry", Career_Parse_UpgradeDepartmentMoraleEntry_t> Career_Parse_UpgradeDepartmentMoraleEntry{}; // L2 experimental
using Career_Parse_UpgradeMoraleAdjustmentEffect_t = void **(void **out, void *allocator, void *unused, void *xml_node);
inline constexpr fn<"Career_Parse_UpgradeMoraleAdjustmentEffect", Career_Parse_UpgradeMoraleAdjustmentEffect_t> Career_Parse_UpgradeMoraleAdjustmentEffect{}; // L2 experimental
inline constexpr address<"Career_Pick_RandomUpgrade"> Career_Pick_RandomUpgrade{}; // L2 experimental
using Career_Pick_RandomUpgrade_A_t = uint64_t(void *self);
inline constexpr fn<"Career_Pick_RandomUpgrade_A", Career_Pick_RandomUpgrade_A_t> Career_Pick_RandomUpgrade_A{}; // L2 experimental
using Career_Pick_RandomUpgrade_B_t = void(uint64_t *self, int64_t *a2);
inline constexpr fn<"Career_Pick_RandomUpgrade_B", Career_Pick_RandomUpgrade_B_t> Career_Pick_RandomUpgrade_B{}; // L2 experimental
using Career_Pick_RandomUpgrade_C_t = uint64_t *(uint64_t *self, void *a2, void *a3);
inline constexpr fn<"Career_Pick_RandomUpgrade_C", Career_Pick_RandomUpgrade_C_t> Career_Pick_RandomUpgrade_C{}; // L2 experimental
using Career_Populate_RnDAdvanceResults_t = void(void *self, uint32_t a2);
inline constexpr fn<"Career_Populate_RnDAdvanceResults", Career_Populate_RnDAdvanceResults_t> Career_Populate_RnDAdvanceResults{}; // L2 experimental
using Career_Post_DepartmentEventOutcomeChosen_t = void(void *self);
inline constexpr fn<"Career_Post_DepartmentEventOutcomeChosen", Career_Post_DepartmentEventOutcomeChosen_t> Career_Post_DepartmentEventOutcomeChosen{}; // L2 experimental
using Career_Post_RnDUpgradePurchaseError_t = void(void *self, void *a2, uint32_t a3, void *a4, void *a5);
inline constexpr fn<"Career_Post_RnDUpgradePurchaseError", Career_Post_RnDUpgradePurchaseError_t> Career_Post_RnDUpgradePurchaseError{}; // L2 experimental
using Career_Post_UpgradeFittedRatio_t = void(void *upgrades, void *inst, uint8_t fitted);
inline constexpr fn<"Career_Post_UpgradeFittedRatio", Career_Post_UpgradeFittedRatio_t> Career_Post_UpgradeFittedRatio{}; // L2 experimental
inline constexpr address<"Career_PressOutcome_BuildRewardLines"> Career_PressOutcome_BuildRewardLines{}; // L2 experimental
inline constexpr address<"Career_PrimarySponsorGoalPanel_Build"> Career_PrimarySponsorGoalPanel_Build{}; // L2 experimental
using Career_Purchase_Upgrade_Sim_t = void(void *self);
inline constexpr fn<"Career_Purchase_Upgrade_Sim", Career_Purchase_Upgrade_Sim_t> Career_Purchase_Upgrade_Sim{}; // L2 experimental
using Career_Purchase_UpgradesForSimulatedTeams_t = void(int64_t *self);
inline constexpr fn<"Career_Purchase_UpgradesForSimulatedTeams", Career_Purchase_UpgradesForSimulatedTeams_t> Career_Purchase_UpgradesForSimulatedTeams{}; // L2 experimental
using Career_Read_AdaptableUpgradeCounts_t = int(uint64_t *self, uint64_t a2);
inline constexpr fn<"Career_Read_AdaptableUpgradeCounts", Career_Read_AdaptableUpgradeCounts_t> Career_Read_AdaptableUpgradeCounts{}; // L2 experimental
using Career_Read_AwardedUpgradeReward_t = int(void *self, uint64_t a2);
inline constexpr fn<"Career_Read_AwardedUpgradeReward", Career_Read_AwardedUpgradeReward_t> Career_Read_AwardedUpgradeReward{}; // L2 experimental
inline constexpr address<"Career_Read_CareerSetupDataConfig"> Career_Read_CareerSetupDataConfig{}; // L2 experimental
inline constexpr address<"Career_Read_CashFacilityValuesConfig"> Career_Read_CashFacilityValuesConfig{}; // L2 experimental
using Career_Read_DepartmentSchedules_t = int(uint64_t self, uint64_t a2);
inline constexpr fn<"Career_Read_DepartmentSchedules", Career_Read_DepartmentSchedules_t> Career_Read_DepartmentSchedules{}; // L2 experimental
inline constexpr address<"Career_Read_FacilityPerkTreeConfig"> Career_Read_FacilityPerkTreeConfig{}; // L2 experimental
inline constexpr address<"Career_Read_FacilityUpgradeNodeConfig"> Career_Read_FacilityUpgradeNodeConfig{}; // L2 experimental
using Career_Read_PerDriverUniqueRecommendedUpgrades_t = int(void *self, uint64_t a2, uint64_t a3);
inline constexpr fn<"Career_Read_PerDriverUniqueRecommendedUpgrades", Career_Read_PerDriverUniqueRecommendedUpgrades_t> Career_Read_PerDriverUniqueRecommendedUpgrades{}; // L2 experimental
inline constexpr address<"Career_Read_PointsFacilityValuesConfig"> Career_Read_PointsFacilityValuesConfig{}; // L2 experimental
inline constexpr address<"Career_Read_QuickPracticeConfig"> Career_Read_QuickPracticeConfig{}; // L2 experimental
using Career_Read_RegulationAdaptRange_t = int(uint32_t *self, uint64_t a2);
inline constexpr fn<"Career_Read_RegulationAdaptRange", Career_Read_RegulationAdaptRange_t> Career_Read_RegulationAdaptRange{}; // L2 experimental
using Career_Read_RegulationAdaptRange_Alt_t = int(uint64_t *self, uint64_t a2);
inline constexpr fn<"Career_Read_RegulationAdaptRange_Alt", Career_Read_RegulationAdaptRange_Alt_t> Career_Read_RegulationAdaptRange_Alt{}; // L2 experimental
inline constexpr address<"Career_Read_ResourcePointGainRateConfig"> Career_Read_ResourcePointGainRateConfig{}; // L2 experimental
inline constexpr address<"Career_Read_ResourcePointsStartConfig"> Career_Read_ResourcePointsStartConfig{}; // L2 experimental
inline constexpr address<"Career_Read_RndChallengesConfig"> Career_Read_RndChallengesConfig{}; // L2 experimental
inline constexpr address<"Career_Read_SponsorDiscoveryAndFacilityRequestConfig"> Career_Read_SponsorDiscoveryAndFacilityRequestConfig{}; // L2 experimental
using Career_Read_UpgradeHashname_t = int(int64_t *self, uint64_t a2, uint64_t a3, void *a4);
inline constexpr fn<"Career_Read_UpgradeHashname", Career_Read_UpgradeHashname_t> Career_Read_UpgradeHashname{}; // L2 experimental
using Career_Read_UpgradeHashnameList_t = int(uint64_t *self, uint64_t a2, uint64_t a3, uint64_t a4);
inline constexpr fn<"Career_Read_UpgradeHashnameList", Career_Read_UpgradeHashnameList_t> Career_Read_UpgradeHashnameList{}; // L2 experimental
using Career_Read_UpgradesList_t = int(void *self, uint64_t a2, uint64_t a3);
inline constexpr fn<"Career_Read_UpgradesList", Career_Read_UpgradesList_t> Career_Read_UpgradesList{}; // L2 experimental
using Career_Rebuild_ChampionshipStandings_t = void(void *champ_list, void *extra_points, void *extra_e);
inline constexpr fn<"Career_Rebuild_ChampionshipStandings", Career_Rebuild_ChampionshipStandings_t> Career_Rebuild_ChampionshipStandings{}; // L2 experimental
using Career_Rebuild_SeasonStandings_t = void(void *season, void *order_a, void *order_b, void *extra_points, void *extra_e);
inline constexpr fn<"Career_Rebuild_SeasonStandings", Career_Rebuild_SeasonStandings_t> Career_Rebuild_SeasonStandings{}; // L2 experimental
using Career_Record_DriverMove_t = void(void *renewal, uint32_t driver, uint32_t old_team, uint32_t new_team, uint8_t reason);
inline constexpr fn<"Career_Record_DriverMove", Career_Record_DriverMove_t> Career_Record_DriverMove{}; // L2 experimental
using Career_Refresh_RnDDeptMode_t = void(uintptr_t *holder, uint32_t mode);
inline constexpr fn<"Career_Refresh_RnDDeptMode", Career_Refresh_RnDDeptMode_t> Career_Refresh_RnDDeptMode{}; // L2 experimental
using Career_Refresh_UpgradeAvailability_t = void(void *upgrades, uint16_t day);
inline constexpr fn<"Career_Refresh_UpgradeAvailability", Career_Refresh_UpgradeAvailability_t> Career_Refresh_UpgradeAvailability{}; // L2 experimental
using Career_Register_RegistryEntry_t = uint64_t(void *map, void *key, void *entry);
inline constexpr fn<"Career_Register_RegistryEntry", Career_Register_RegistryEntry_t> Career_Register_RegistryEntry{}; // L2 experimental
inline constexpr address<"Career_Register_TeamHqMyTeamScreens"> Career_Register_TeamHqMyTeamScreens{}; // L2 experimental
inline constexpr address<"Career_Remove_ActivitiesByKindUpToLevel"> Career_Remove_ActivitiesByKindUpToLevel{}; // L2 experimental
inline constexpr address<"Career_RenownData_ComputeFormLevel"> Career_RenownData_ComputeFormLevel{}; // L2 experimental
inline constexpr address<"Career_RenownData_Deserialise"> Career_RenownData_Deserialise{}; // L2 experimental
inline constexpr address<"Career_RenownData_Serialise"> Career_RenownData_Serialise{}; // L2 experimental
inline constexpr address<"Career_RenownFormDetails_BuildPills"> Career_RenownFormDetails_BuildPills{}; // L2 experimental
inline constexpr address<"Career_RenownFormScreen_OnActivate"> Career_RenownFormScreen_OnActivate{}; // L2 experimental
inline constexpr address<"Career_RenownFormSmallWidget_Update"> Career_RenownFormSmallWidget_Update{}; // L2 experimental
inline constexpr address<"Career_RenownFormVM_Init"> Career_RenownFormVM_Init{}; // L2 experimental
inline constexpr address<"Career_RenownForm_BuildLevelName"> Career_RenownForm_BuildLevelName{}; // L2 experimental
inline constexpr address<"Career_RenownForm_BuildNextTierGrantText"> Career_RenownForm_BuildNextTierGrantText{}; // L2 experimental
inline constexpr address<"Career_RenownForm_BuildPaddockGossip"> Career_RenownForm_BuildPaddockGossip{}; // L2 experimental
inline constexpr address<"Career_RenownForm_BuildRenownText"> Career_RenownForm_BuildRenownText{}; // L2 experimental
inline constexpr address<"Career_RenownForm_GetOverrideKeys"> Career_RenownForm_GetOverrideKeys{}; // L2 experimental
inline constexpr address<"Career_RenownForm_SumPointsByKeyAndKind"> Career_RenownForm_SumPointsByKeyAndKind{}; // L2 experimental
inline constexpr address<"Career_RenownGraph_ReadPoints"> Career_RenownGraph_ReadPoints{}; // L2 experimental
inline constexpr address<"Career_RenownGraph_WritePoints"> Career_RenownGraph_WritePoints{}; // L2 experimental
inline constexpr address<"Career_RenownIncreasedPopup_Init"> Career_RenownIncreasedPopup_Init{}; // L2 experimental
inline constexpr address<"Career_RenownIncreasedPopup_Update"> Career_RenownIncreasedPopup_Update{}; // L2 experimental
inline constexpr address<"Career_RenownLabel_BuildData"> Career_RenownLabel_BuildData{}; // L2 experimental
inline constexpr address<"Career_RenownLevelUpVM_Init"> Career_RenownLevelUpVM_Init{}; // L2 experimental
inline constexpr address<"Career_RenownLevelUpVM_OnAction"> Career_RenownLevelUpVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_RenownLevelUpVM_SetLevelIcon"> Career_RenownLevelUpVM_SetLevelIcon{}; // L2 experimental
inline constexpr address<"Career_RenownWeight_ReadFields"> Career_RenownWeight_ReadFields{}; // L2 experimental
inline constexpr address<"Career_RenownWeight_WriteFields"> Career_RenownWeight_WriteFields{}; // L2 experimental
inline constexpr address<"Career_Report_RndPointsFilter"> Career_Report_RndPointsFilter{}; // L2 experimental
inline constexpr address<"Career_ResourcePointsAward_ReadSaveData"> Career_ResourcePointsAward_ReadSaveData{}; // L2 experimental
inline constexpr address<"Career_ResourcePointsAward_WriteSaveData"> Career_ResourcePointsAward_WriteSaveData{}; // L2 experimental
using Career_ResourcePoints_OnAwardToKeyMessage_t = uint64_t(void *rp, void *msg);
inline constexpr fn<"Career_ResourcePoints_OnAwardToKeyMessage", Career_ResourcePoints_OnAwardToKeyMessage_t> Career_ResourcePoints_OnAwardToKeyMessage{}; // L2 experimental
using Career_ResourcePoints_OnAwardToMemberMessage_t = uint64_t(void *rp, void *msg);
inline constexpr fn<"Career_ResourcePoints_OnAwardToMemberMessage", Career_ResourcePoints_OnAwardToMemberMessage_t> Career_ResourcePoints_OnAwardToMemberMessage{}; // L2 experimental
using Career_ResourcePoints_OnGainRateStatChanged_t = void(void *rp);
inline constexpr fn<"Career_ResourcePoints_OnGainRateStatChanged", Career_ResourcePoints_OnGainRateStatChanged_t> Career_ResourcePoints_OnGainRateStatChanged{}; // L2 experimental
using Career_ResourcePoints_OnRndChallengeAppliedMessage_t = uint64_t(void *rp, void *msg);
inline constexpr fn<"Career_ResourcePoints_OnRndChallengeAppliedMessage", Career_ResourcePoints_OnRndChallengeAppliedMessage_t> Career_ResourcePoints_OnRndChallengeAppliedMessage{}; // L2 experimental
using Career_ResourcePoints_OnSlotAwardMessage_t = uint64_t(void *rp, void *msg);
inline constexpr fn<"Career_ResourcePoints_OnSlotAwardMessage", Career_ResourcePoints_OnSlotAwardMessage_t> Career_ResourcePoints_OnSlotAwardMessage{}; // L2 experimental
using Career_ResourcePoints_OnTeamWalletDeltaMessage_t = uint64_t(void *rp, void *msg);
inline constexpr fn<"Career_ResourcePoints_OnTeamWalletDeltaMessage", Career_ResourcePoints_OnTeamWalletDeltaMessage_t> Career_ResourcePoints_OnTeamWalletDeltaMessage{}; // L2 experimental
using Career_ResourcePoints_OnUnlockFlagMessage_t = uint64_t(void *rp, void *msg);
inline constexpr fn<"Career_ResourcePoints_OnUnlockFlagMessage", Career_ResourcePoints_OnUnlockFlagMessage_t> Career_ResourcePoints_OnUnlockFlagMessage{}; // L2 experimental
inline constexpr address<"Career_ResourcePoints_UnsubscribeMessages"> Career_ResourcePoints_UnsubscribeMessages{}; // L2 experimental
inline constexpr address<"Career_RewardsScreen_BuildAcclaimAwarded"> Career_RewardsScreen_BuildAcclaimAwarded{}; // L2 experimental
inline constexpr address<"Career_SaveBlobCurve_CopyAssign"> Career_SaveBlobCurve_CopyAssign{}; // L2 experimental
inline constexpr address<"Career_SaveBlobCurve_CopyConstruct"> Career_SaveBlobCurve_CopyConstruct{}; // L2 experimental
inline constexpr address<"Career_SaveBlobCurve_Serialise"> Career_SaveBlobCurve_Serialise{}; // L2 experimental
inline constexpr address<"Career_Save_DeptEventState"> Career_Save_DeptEventState{}; // L2 experimental
inline constexpr address<"Career_Save_RndChallengeState"> Career_Save_RndChallengeState{}; // L2 experimental
inline constexpr address<"Career_Save_RndHeads"> Career_Save_RndHeads{}; // L2 experimental
using Career_Save_TeamDepartmentsState_t = uint64_t(void *team_comp, void *save_node, int64_t version);
inline constexpr fn<"Career_Save_TeamDepartmentsState", Career_Save_TeamDepartmentsState_t> Career_Save_TeamDepartmentsState{}; // L2 experimental
inline constexpr address<"Career_Save_UpgradeLevelFields"> Career_Save_UpgradeLevelFields{}; // L2 experimental
inline constexpr address<"Career_ScheduleEntryDeptEvent_Start"> Career_ScheduleEntryDeptEvent_Start{}; // L2 experimental
inline constexpr address<"Career_ScheduleEntry_Finish"> Career_ScheduleEntry_Finish{}; // L2 experimental
inline constexpr address<"Career_ScheduleEntry_TryStart"> Career_ScheduleEntry_TryStart{}; // L2 experimental
inline constexpr address<"Career_SecretMeetingsVM_OnScreenDeactivated"> Career_SecretMeetingsVM_OnScreenDeactivated{}; // L2 experimental
using Career_Select_DeluxeUpgradeBundleLabel_t = void(void *self, uint64_t *a2);
inline constexpr fn<"Career_Select_DeluxeUpgradeBundleLabel", Career_Select_DeluxeUpgradeBundleLabel_t> Career_Select_DeluxeUpgradeBundleLabel{}; // L2 experimental
inline constexpr address<"Career_Select_FacilityUpgrade"> Career_Select_FacilityUpgrade{}; // L2 experimental
inline constexpr address<"Career_Select_MyTeamSetupQuestion"> Career_Select_MyTeamSetupQuestion{}; // L2 experimental
using Career_Select_RnDPlayerControlDept_t = void(void *self);
inline constexpr fn<"Career_Select_RnDPlayerControlDept", Career_Select_RnDPlayerControlDept_t> Career_Select_RnDPlayerControlDept{}; // L2 experimental
using Career_Select_RnDReturnDepartment_t = uint64_t(void *self, uint64_t *a2);
inline constexpr fn<"Career_Select_RnDReturnDepartment", Career_Select_RnDReturnDepartment_t> Career_Select_RnDReturnDepartment{}; // L2 experimental
inline constexpr address<"Career_Select_TopLevelFacility"> Career_Select_TopLevelFacility{}; // L2 experimental
using Career_Set_DriverContractState_t = void(void *contract, uint16_t day, uint8_t state, uint32_t value);
inline constexpr fn<"Career_Set_DriverContractState", Career_Set_DriverContractState_t> Career_Set_DriverContractState{}; // L2 experimental
using Career_Set_EntrantData_Thunk_t = void(void *entity, const void *entrant_id, const uint32_t *data, uint16_t position, uint8_t flag);
inline constexpr fn<"Career_Set_EntrantData_Thunk", Career_Set_EntrantData_Thunk_t> Career_Set_EntrantData_Thunk{}; // L2 experimental
inline constexpr address<"Career_Set_FacilityPhaseLabel"> Career_Set_FacilityPhaseLabel{}; // L2 experimental
using Career_Set_RnDDualDevelopSelected_t = uint64_t(int64_t *self, uint8_t a2);
inline constexpr fn<"Career_Set_RnDDualDevelopSelected", Career_Set_RnDDualDevelopSelected_t> Career_Set_RnDDualDevelopSelected{}; // L2 experimental
using Career_Set_RnDFocusedDepartment_t = uint64_t(void *self);
inline constexpr fn<"Career_Set_RnDFocusedDepartment", Career_Set_RnDFocusedDepartment_t> Career_Set_RnDFocusedDepartment{}; // L2 experimental
using Career_Set_UpgradeFacilityLocked_t = void(void *inst, uint8_t locked, uint16_t day);
inline constexpr fn<"Career_Set_UpgradeFacilityLocked", Career_Set_UpgradeFacilityLocked_t> Career_Set_UpgradeFacilityLocked{}; // L2 experimental
using Career_Set_UpgradesOwner_t = void(void *upgrades, uint32_t owner);
inline constexpr fn<"Career_Set_UpgradesOwner", Career_Set_UpgradesOwner_t> Career_Set_UpgradesOwner{}; // L2 experimental
inline constexpr address<"Career_SetupBanks_Write"> Career_SetupBanks_Write{}; // L2 experimental
inline constexpr address<"Career_SetupDataConfig_CopyConstruct"> Career_SetupDataConfig_CopyConstruct{}; // L2 experimental
using Career_Sign_DriverContract_t = void(void *contracts, void *contract, uint64_t driver_key, uint32_t replaced_key, uint8_t notify);
inline constexpr fn<"Career_Sign_DriverContract", Career_Sign_DriverContract_t> Career_Sign_DriverContract{}; // L2 experimental
inline constexpr address<"Career_Sign_DriverContractIfNotTeam"> Career_Sign_DriverContractIfNotTeam{}; // L2 experimental
inline constexpr address<"Career_Specialists_CheckSelectionSameTeam"> Career_Specialists_CheckSelectionSameTeam{}; // L2 experimental
inline constexpr address<"Career_SponsorCard_BuildData"> Career_SponsorCard_BuildData{}; // L2 experimental
inline constexpr address<"Career_SponsorContractRiskOverlay_Update"> Career_SponsorContractRiskOverlay_Update{}; // L2 experimental
inline constexpr address<"Career_SponsorContractSentVM_OnAction"> Career_SponsorContractSentVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_SponsorContractsVM_BuildOfferCards"> Career_SponsorContractsVM_BuildOfferCards{}; // L2 experimental
inline constexpr address<"Career_SponsorContractsVM_OnAction"> Career_SponsorContractsVM_OnAction{}; // L2 experimental
using Career_SponsorData_CopyConstruct_t = void(void *dst, const void *src);
inline constexpr fn<"Career_SponsorData_CopyConstruct", Career_SponsorData_CopyConstruct_t> Career_SponsorData_CopyConstruct{}; // L2 experimental
inline constexpr address<"Career_SponsorGoal_GetScaledRewardD4"> Career_SponsorGoal_GetScaledRewardD4{}; // L2 experimental
inline constexpr address<"Career_SponsorLevelNotificationVM_Init"> Career_SponsorLevelNotificationVM_Init{}; // L2 experimental
inline constexpr address<"Career_SponsorRenewalCounterofferVM_OnActivate"> Career_SponsorRenewalCounterofferVM_OnActivate{}; // L2 experimental
inline constexpr address<"Career_SponsorRenewalDetailsVM_Build"> Career_SponsorRenewalDetailsVM_Build{}; // L2 experimental
inline constexpr address<"Career_SponsorUpdateVM_Build"> Career_SponsorUpdateVM_Build{}; // L2 experimental
inline constexpr address<"Career_SponsorUpdateVM_OnAction"> Career_SponsorUpdateVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_Sponsor_GetModifierMultiplierForTeam"> Career_Sponsor_GetModifierMultiplierForTeam{}; // L2 experimental
inline constexpr address<"Career_SponsorsHqCard_GetText"> Career_SponsorsHqCard_GetText{}; // L2 experimental
inline constexpr address<"Career_SponsorsVM_OnCommandRaised"> Career_SponsorsVM_OnCommandRaised{}; // L2 experimental
inline constexpr address<"Career_SponsorsVM_Refresh"> Career_SponsorsVM_Refresh{}; // L2 experimental
using Career_Sponsors_AppendRecordField_t = void(void **vec, void *rec);
inline constexpr fn<"Career_Sponsors_AppendRecordField", Career_Sponsors_AppendRecordField_t> Career_Sponsors_AppendRecordField{}; // L2 experimental
inline constexpr address<"Career_Sponsors_DeserialiseGoals"> Career_Sponsors_DeserialiseGoals{}; // L2 experimental
inline constexpr address<"Career_Sponsors_SerialiseGoalDriverIds"> Career_Sponsors_SerialiseGoalDriverIds{}; // L2 experimental
inline constexpr address<"Career_Sponsors_SerialiseLoyaltyTrack"> Career_Sponsors_SerialiseLoyaltyTrack{}; // L2 experimental
inline constexpr address<"Career_Sponsors_SumGoalRewardsForTeam"> Career_Sponsors_SumGoalRewardsForTeam{}; // L2 experimental
inline constexpr address<"Career_Sponsors_WriteSaveTree"> Career_Sponsors_WriteSaveTree{}; // L2 experimental
using Career_Start_UpgradeDevelopment_t = void(void *upgrades, uint32_t owner, void *inst, uint16_t start_day, uint16_t end_day, uint8_t option, uint8_t dual, uint8_t roll_failure, void *rng, uint8_t store_duration);
inline constexpr fn<"Career_Start_UpgradeDevelopment", Career_Start_UpgradeDevelopment_t> Career_Start_UpgradeDevelopment{}; // L2 experimental
using Career_Start_UpgradeResearch_t = void(void *upgrades, uint32_t owner, void *inst, uint32_t start_day, uint16_t end_day, uint8_t option, uint8_t roll_failure, void *rng, uint8_t store_duration);
inline constexpr fn<"Career_Start_UpgradeResearch", Career_Start_UpgradeResearch_t> Career_Start_UpgradeResearch{}; // L2 experimental
inline constexpr address<"Career_StateTeamHq_OnStateExit"> Career_StateTeamHq_OnStateExit{}; // L2 experimental
inline constexpr address<"Career_Sum_ContractSlotFloatValues"> Career_Sum_ContractSlotFloatValues{}; // L2 experimental
using Career_Switch_RnDResearchMode_t = void(void *self);
inline constexpr fn<"Career_Switch_RnDResearchMode", Career_Switch_RnDResearchMode_t> Career_Switch_RnDResearchMode{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvanceVM_AllowOnlineUnready"> Career_TeamHqAdvanceVM_AllowOnlineUnready{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvanceVM_EnterPressInterview"> Career_TeamHqAdvanceVM_EnterPressInterview{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvanceVM_Initialise"> Career_TeamHqAdvanceVM_Initialise{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvanceVM_OnNotificationsAnimOff"> Career_TeamHqAdvanceVM_OnNotificationsAnimOff{}; // L2 experimental
using Career_TeamHqAdvance_BuildAdvanceCards_t = void(void *advance, void *out);
inline constexpr fn<"Career_TeamHqAdvance_BuildAdvanceCards", Career_TeamHqAdvance_BuildAdvanceCards_t> Career_TeamHqAdvance_BuildAdvanceCards{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvance_CheckContractedDriverActivities"> Career_TeamHqAdvance_CheckContractedDriverActivities{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvance_ClearDelayedByAnims"> Career_TeamHqAdvance_ClearDelayedByAnims{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvance_CollectAdvanceEntries"> Career_TeamHqAdvance_CollectAdvanceEntries{}; // L2 experimental
using Career_TeamHqAdvance_GetStopTimeKey_t = void **(void *advance, void **out_key);
inline constexpr fn<"Career_TeamHqAdvance_GetStopTimeKey", Career_TeamHqAdvance_GetStopTimeKey_t> Career_TeamHqAdvance_GetStopTimeKey{}; // L2 experimental
using Career_TeamHqAdvance_IsCardActionVisible_t = bool(void *advance, void *card, void *action);
inline constexpr fn<"Career_TeamHqAdvance_IsCardActionVisible", Career_TeamHqAdvance_IsCardActionVisible_t> Career_TeamHqAdvance_IsCardActionVisible{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvance_RestartTimer"> Career_TeamHqAdvance_RestartTimer{}; // L2 experimental
inline constexpr address<"Career_TeamHqAdvance_SetAdvanceRequested"> Career_TeamHqAdvance_SetAdvanceRequested{}; // L2 experimental
inline constexpr address<"Career_TeamHqContainerVM_OnAction"> Career_TeamHqContainerVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_TeamHqContainerVM_OnScreenActivated"> Career_TeamHqContainerVM_OnScreenActivated{}; // L2 experimental
using Career_TeamHqContractsVM_ClearRows_t = void(void *vm);
inline constexpr fn<"Career_TeamHqContractsVM_ClearRows", Career_TeamHqContractsVM_ClearRows_t> Career_TeamHqContractsVM_ClearRows{}; // L2 experimental
using Career_TeamHqContractsVM_OnActivate_t = void(void *vm);
inline constexpr fn<"Career_TeamHqContractsVM_OnActivate", Career_TeamHqContractsVM_OnActivate_t> Career_TeamHqContractsVM_OnActivate{}; // L2 experimental
using Career_TeamHqContractsVM_OnDeactivate_t = void(void *vm);
inline constexpr fn<"Career_TeamHqContractsVM_OnDeactivate", Career_TeamHqContractsVM_OnDeactivate_t> Career_TeamHqContractsVM_OnDeactivate{}; // L2 experimental
using Career_TeamHqContractsVM_OnOpenContractNegotiations_t = void *(void *vm, void **event);
inline constexpr fn<"Career_TeamHqContractsVM_OnOpenContractNegotiations", Career_TeamHqContractsVM_OnOpenContractNegotiations_t> Career_TeamHqContractsVM_OnOpenContractNegotiations{}; // L2 experimental
using Career_TeamHqContractsVM_OnUiEvent_t = uint64_t(void *vm, void *event);
inline constexpr fn<"Career_TeamHqContractsVM_OnUiEvent", Career_TeamHqContractsVM_OnUiEvent_t> Career_TeamHqContractsVM_OnUiEvent{}; // L2 experimental
using Career_TeamHqContractsVM_RebuildRows_t = void(void *vm);
inline constexpr fn<"Career_TeamHqContractsVM_RebuildRows", Career_TeamHqContractsVM_RebuildRows_t> Career_TeamHqContractsVM_RebuildRows{}; // L2 experimental
using Career_TeamHqDriverContractsView_BindWidgets_t = void(void *view);
inline constexpr fn<"Career_TeamHqDriverContractsView_BindWidgets", Career_TeamHqDriverContractsView_BindWidgets_t> Career_TeamHqDriverContractsView_BindWidgets{}; // L2 experimental
inline constexpr address<"Career_TeamHqDriverRender_Init"> Career_TeamHqDriverRender_Init{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_CameraCycleFadeIn"> Career_TeamHqFacilities_CameraCycleFadeIn{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_CameraCycleFadeInNamed"> Career_TeamHqFacilities_CameraCycleFadeInNamed{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_CameraCycleFadeInOnEnter"> Career_TeamHqFacilities_CameraCycleFadeInOnEnter{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_CameraCycleStep"> Career_TeamHqFacilities_CameraCycleStep{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_ReadCameraCycleSettings"> Career_TeamHqFacilities_ReadCameraCycleSettings{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_ReadFacilitiesSelectionConfig"> Career_TeamHqFacilities_ReadFacilitiesSelectionConfig{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_ReadSaveData"> Career_TeamHqFacilities_ReadSaveData{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_UpdateCameraAndPlayerControl"> Career_TeamHqFacilities_UpdateCameraAndPlayerControl{}; // L2 experimental
inline constexpr address<"Career_TeamHqFacilities_WriteSaveData"> Career_TeamHqFacilities_WriteSaveData{}; // L2 experimental
inline constexpr address<"Career_TeamHqHub_BuildHomeButtons"> Career_TeamHqHub_BuildHomeButtons{}; // L2 experimental
inline constexpr address<"Career_TeamHqLaptopVM_OnAction"> Career_TeamHqLaptopVM_OnAction{}; // L2 experimental
inline constexpr address<"Career_TeamHqLaptopVM_OnDeactivate"> Career_TeamHqLaptopVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_ClearStopping"> Career_TeamHqMyTeamVM_ClearStopping{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_GetSponsorsData"> Career_TeamHqMyTeamVM_GetSponsorsData{}; // L2 experimental
using Career_TeamHqMyTeamVM_GotoDriverCareerContractNegotiations_t = uint64_t(void *vm);
inline constexpr fn<"Career_TeamHqMyTeamVM_GotoDriverCareerContractNegotiations", Career_TeamHqMyTeamVM_GotoDriverCareerContractNegotiations_t> Career_TeamHqMyTeamVM_GotoDriverCareerContractNegotiations{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_Initialise"> Career_TeamHqMyTeamVM_Initialise{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_OnCommandRaised"> Career_TeamHqMyTeamVM_OnCommandRaised{}; // L2 experimental
using Career_TeamHqMyTeamVM_OnContractNegotiationsRequest_t = uint64_t(void *vm, void *request);
inline constexpr fn<"Career_TeamHqMyTeamVM_OnContractNegotiationsRequest", Career_TeamHqMyTeamVM_OnContractNegotiationsRequest_t> Career_TeamHqMyTeamVM_OnContractNegotiationsRequest{}; // L2 experimental
using Career_TeamHqMyTeamVM_OnDepartmentEventOutcomeChosen_t = void(void *vm, void *msg);
inline constexpr fn<"Career_TeamHqMyTeamVM_OnDepartmentEventOutcomeChosen", Career_TeamHqMyTeamVM_OnDepartmentEventOutcomeChosen_t> Career_TeamHqMyTeamVM_OnDepartmentEventOutcomeChosen{}; // L2 experimental
using Career_TeamHqMyTeamVM_OnMenuTransition_t = long *(void *vm, void *event, void *arg);
inline constexpr fn<"Career_TeamHqMyTeamVM_OnMenuTransition", Career_TeamHqMyTeamVM_OnMenuTransition_t> Career_TeamHqMyTeamVM_OnMenuTransition{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_OnTimer"> Career_TeamHqMyTeamVM_OnTimer{}; // L2 experimental
using Career_TeamHqMyTeamVM_OpenContractNegotiations_t = uint64_t(void *vm);
inline constexpr fn<"Career_TeamHqMyTeamVM_OpenContractNegotiations", Career_TeamHqMyTeamVM_OpenContractNegotiations_t> Career_TeamHqMyTeamVM_OpenContractNegotiations{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_SignalStartAdvancing"> Career_TeamHqMyTeamVM_SignalStartAdvancing{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_StopAdvancing"> Career_TeamHqMyTeamVM_StopAdvancing{}; // L2 experimental
inline constexpr address<"Career_TeamHqMyTeamVM_Update"> Career_TeamHqMyTeamVM_Update{}; // L2 experimental
inline constexpr address<"Career_TeamHqPopup_BuildSponsorNegotiationCard"> Career_TeamHqPopup_BuildSponsorNegotiationCard{}; // L2 experimental
inline constexpr address<"Career_TeamHqRender_ResolveServices"> Career_TeamHqRender_ResolveServices{}; // L2 experimental
inline constexpr address<"Career_TeamHqState_ReadResourcePoints"> Career_TeamHqState_ReadResourcePoints{}; // L2 experimental
inline constexpr address<"Career_TeamHqState_WriteResourcePoints"> Career_TeamHqState_WriteResourcePoints{}; // L2 experimental
inline constexpr address<"Career_TeamHq_GetRenownFormTab"> Career_TeamHq_GetRenownFormTab{}; // L2 experimental
using Career_TeamHq_SetContractNegotiationsLaunchedViaAdvance_t = void(bool via_advance);
inline constexpr fn<"Career_TeamHq_SetContractNegotiationsLaunchedViaAdvance", Career_TeamHq_SetContractNegotiationsLaunchedViaAdvance_t> Career_TeamHq_SetContractNegotiationsLaunchedViaAdvance{}; // L2 experimental
inline constexpr address<"Career_TeamRenownAndForm_Deserialise"> Career_TeamRenownAndForm_Deserialise{}; // L2 experimental
inline constexpr address<"Career_TeamRenownAndForm_Serialise"> Career_TeamRenownAndForm_Serialise{}; // L2 experimental
using Career_Teardown_Weekend_t = void(void *weekend);
inline constexpr fn<"Career_Teardown_Weekend", Career_Teardown_Weekend_t> Career_Teardown_Weekend{}; // L2 experimental
using Career_Timeline_FindEntryFromDay_t = void *(void *vm, char want_kind, uint16_t from_day, uint8_t inclusive);
inline constexpr fn<"Career_Timeline_FindEntryFromDay", Career_Timeline_FindEntryFromDay_t> Career_Timeline_FindEntryFromDay{}; // L2 experimental
inline constexpr address<"Career_TitleSponsorsLoyaltyRewardsVM_OnAction"> Career_TitleSponsorsLoyaltyRewardsVM_OnAction{}; // L2 experimental
using Career_Update_DepartmentMoraleStateVM_t = void(void *vm);
inline constexpr fn<"Career_Update_DepartmentMoraleStateVM", Career_Update_DepartmentMoraleStateVM_t> Career_Update_DepartmentMoraleStateVM{}; // L2 experimental
inline constexpr address<"Career_Update_FacilityUpgradeButton"> Career_Update_FacilityUpgradeButton{}; // L2 experimental
inline constexpr address<"Career_Update_FacilityUpgradeInfoPanel"> Career_Update_FacilityUpgradeInfoPanel{}; // L2 experimental
inline constexpr address<"Career_Update_RnDAdaptPartPanel"> Career_Update_RnDAdaptPartPanel{}; // L2 experimental
inline constexpr address<"Career_Update_RnDDepartmentSummary"> Career_Update_RnDDepartmentSummary{}; // L2 experimental
using Career_Update_RnDDevelopSlotCard_t = void(void *self, char a2);
inline constexpr fn<"Career_Update_RnDDevelopSlotCard", Career_Update_RnDDevelopSlotCard_t> Career_Update_RnDDevelopSlotCard{}; // L2 experimental
using Career_Update_RnDDriverUpgradeRow_t = uint64_t(void *self);
inline constexpr fn<"Career_Update_RnDDriverUpgradeRow", Career_Update_RnDDriverUpgradeRow_t> Career_Update_RnDDriverUpgradeRow{}; // L2 experimental
using Career_Update_RnDEquipToDriverButton_A_t = uint64_t(void *self, char a2);
inline constexpr fn<"Career_Update_RnDEquipToDriverButton_A", Career_Update_RnDEquipToDriverButton_A_t> Career_Update_RnDEquipToDriverButton_A{}; // L2 experimental
using Career_Update_RnDEquipToDriverButton_B_t = uint64_t(void *self, char a2);
inline constexpr fn<"Career_Update_RnDEquipToDriverButton_B", Career_Update_RnDEquipToDriverButton_B_t> Career_Update_RnDEquipToDriverButton_B{}; // L2 experimental
using Career_Update_RnDFitUpgradeRow_t = uint64_t(void *self);
inline constexpr fn<"Career_Update_RnDFitUpgradeRow", Career_Update_RnDFitUpgradeRow_t> Career_Update_RnDFitUpgradeRow{}; // L2 experimental
using Career_Update_RnDNumEquippedUpgrades_t = void(int64_t *self, char a2);
inline constexpr fn<"Career_Update_RnDNumEquippedUpgrades", Career_Update_RnDNumEquippedUpgrades_t> Career_Update_RnDNumEquippedUpgrades{}; // L2 experimental
using Career_Update_RnDPartFittingPending_t = uint64_t(void *self);
inline constexpr fn<"Career_Update_RnDPartFittingPending", Career_Update_RnDPartFittingPending_t> Career_Update_RnDPartFittingPending{}; // L2 experimental
inline constexpr address<"Career_Update_RnDPotentialPieTotals"> Career_Update_RnDPotentialPieTotals{}; // L2 experimental
inline constexpr address<"Career_Update_RnDProgressStatusText"> Career_Update_RnDProgressStatusText{}; // L2 experimental
using Career_Update_RnDResearchUpgradeCard_t = void(void *self);
inline constexpr fn<"Career_Update_RnDResearchUpgradeCard", Career_Update_RnDResearchUpgradeCard_t> Career_Update_RnDResearchUpgradeCard{}; // L2 experimental
inline constexpr address<"Career_Update_RnDRushPanel"> Career_Update_RnDRushPanel{}; // L2 experimental
inline constexpr address<"Career_Update_RnDUpgradeCardText"> Career_Update_RnDUpgradeCardText{}; // L2 experimental
inline constexpr address<"Career_UpgradeCondition_CreateCountUpgradeStatus"> Career_UpgradeCondition_CreateCountUpgradeStatus{}; // L2 experimental
inline constexpr address<"Career_UpgradeCondition_CreateUpgradeStatusA"> Career_UpgradeCondition_CreateUpgradeStatusA{}; // L2 experimental
inline constexpr address<"Career_UpgradeCondition_CreateUpgradeStatusB"> Career_UpgradeCondition_CreateUpgradeStatusB{}; // L2 experimental
inline constexpr address<"Career_UpgradeDefinition_CreateFromConfig"> Career_UpgradeDefinition_CreateFromConfig{}; // L2 experimental
inline constexpr address<"Career_UpgradeRef_CreateFromConfig"> Career_UpgradeRef_CreateFromConfig{}; // L2 experimental
inline constexpr address<"Career_Upgrades_CollectEntrantKeys"> Career_Upgrades_CollectEntrantKeys{}; // L2 experimental
inline constexpr address<"Career_Upgrades_FindOrCreateRecommendedState"> Career_Upgrades_FindOrCreateRecommendedState{}; // L2 experimental
inline constexpr address<"Career_WizardTitleSliceVM_OnActivate"> Career_WizardTitleSliceVM_OnActivate{}; // L2 experimental
inline constexpr address<"Career_WizardTitleSliceVM_OnDeactivate"> Career_WizardTitleSliceVM_OnDeactivate{}; // L2 experimental
using Career_Write_AdaptableUpgradeCounts_t = uint64_t(void *self, int64_t *a2);
inline constexpr fn<"Career_Write_AdaptableUpgradeCounts", Career_Write_AdaptableUpgradeCounts_t> Career_Write_AdaptableUpgradeCounts{}; // L2 experimental
using Career_Write_AwardedUpgradeReward_t = int(void *self, int64_t *a2, void *a3);
inline constexpr fn<"Career_Write_AwardedUpgradeReward", Career_Write_AwardedUpgradeReward_t> Career_Write_AwardedUpgradeReward{}; // L2 experimental
using Career_Write_DepartmentSchedules_t = int(uint64_t self, int64_t *a2, void *a3);
inline constexpr fn<"Career_Write_DepartmentSchedules", Career_Write_DepartmentSchedules_t> Career_Write_DepartmentSchedules{}; // L2 experimental
using Career_Write_RegulationAdaptRange_t = void(int *self, uint64_t *a2);
inline constexpr fn<"Career_Write_RegulationAdaptRange", Career_Write_RegulationAdaptRange_t> Career_Write_RegulationAdaptRange{}; // L2 experimental
using Career_Write_RegulationAdaptRange_Alt_t = void(void *self, uint64_t *a2);
inline constexpr fn<"Career_Write_RegulationAdaptRange_Alt", Career_Write_RegulationAdaptRange_Alt_t> Career_Write_RegulationAdaptRange_Alt{}; // L2 experimental
using Career_Write_UpgradeHashname_t = int(int64_t *self, int64_t *a2, uint64_t a3);
inline constexpr fn<"Career_Write_UpgradeHashname", Career_Write_UpgradeHashname_t> Career_Write_UpgradeHashname{}; // L2 experimental
using Career_Write_UpgradesAndAdaptPercent_t = int(void *self, int64_t *a2);
inline constexpr fn<"Career_Write_UpgradesAndAdaptPercent", Career_Write_UpgradesAndAdaptPercent_t> Career_Write_UpgradesAndAdaptPercent{}; // L2 experimental
using Career_Write_UpgradesList_t = int(void *self, int64_t *a2);
inline constexpr fn<"Career_Write_UpgradesList", Career_Write_UpgradesList_t> Career_Write_UpgradesList{}; // L2 experimental
inline constexpr data<"g_Career_FlowLinkContinue", uint32_t> g_Career_FlowLinkContinue{}; // L2 experimental
inline constexpr data<"g_Career_RnDOverviewSelectedDept", uint8_t> g_Career_RnDOverviewSelectedDept{}; // L2 experimental
inline constexpr data<"g_Career_UpgradeLifecycle", uint8_t> g_Career_UpgradeLifecycle{}; // L2 experimental
inline constexpr data<"k_Career_CareerBankComponentDesc", void *> k_Career_CareerBankComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerComponentDesc", void *> k_Career_CareerComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerDriverContractRenewalComponentDesc", void *> k_Career_CareerDriverContractRenewalComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerDriverMarketComponentDesc", void *> k_Career_CareerDriverMarketComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerFacilitiesComponentDesc", void *> k_Career_CareerFacilitiesComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerSetupComponentDesc", void *> k_Career_CareerSetupComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerSponsorsComponentDesc", void *> k_Career_CareerSponsorsComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_CareerTimeComponentDesc", void *> k_Career_CareerTimeComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_ChampionshipComponentDesc", void *> k_Career_ChampionshipComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_ChampionshipDesc", void *> k_Career_ChampionshipDesc{}; // L2 experimental
inline constexpr data<"k_Career_CostCapComponentDesc", void *> k_Career_CostCapComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_DriverAcademiesComponentDesc", void *> k_Career_DriverAcademiesComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_DriverCareerContractsManagerComponentDesc", void *> k_Career_DriverCareerContractsManagerComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_DriverCareerPlayerDriverStatisticsComponentDesc", void *> k_Career_DriverCareerPlayerDriverStatisticsComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_DriverContractsMyTeamComponentDesc", void *> k_Career_DriverContractsMyTeamComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_DriverStatisticsComponentDesc", void *> k_Career_DriverStatisticsComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_EngineSuppliersComponentDesc", void *> k_Career_EngineSuppliersComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_MyTeamAttributesComponentDesc", void *> k_Career_MyTeamAttributesComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_MyTeamFacilitiesComponentDesc", void *> k_Career_MyTeamFacilitiesComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_MyTeamRFComponentDesc", void *> k_Career_MyTeamRFComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_MyTeamStatsComponentDesc", void *> k_Career_MyTeamStatsComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_OwnerPerksComponentDesc", void *> k_Career_OwnerPerksComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_OwnerSetupComponentDesc", void *> k_Career_OwnerSetupComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_ResourcePointsComponentDesc", void *> k_Career_ResourcePointsComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_RivalManagerComponentDesc", void *> k_Career_RivalManagerComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_RnDCardColourTable", uint32_t[6]> k_Career_RnDCardColourTable{}; // L2 experimental
inline constexpr data<"k_Career_RnDDeptColourTable", uint32_t[6]> k_Career_RnDDeptColourTable{}; // L2 experimental
inline constexpr data<"k_Career_RnDErrorDeptName", char[17]> k_Career_RnDErrorDeptName{}; // L2 experimental
inline constexpr data<"k_Career_ScheduleComponentDesc", void *> k_Career_ScheduleComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_TeamHQControllerComponentDesc", void *> k_Career_TeamHQControllerComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_UpgradeBlockNameTable", int32_t[18]> k_Career_UpgradeBlockNameTable{}; // L2 experimental
inline constexpr data<"k_Career_UpgradeTypeOrder", uint8_t[19]> k_Career_UpgradeTypeOrder{}; // L2 experimental
inline constexpr data<"k_Career_UpgradesComponentDesc", void *> k_Career_UpgradesComponentDesc{}; // L2 experimental
inline constexpr data<"k_Career_WeatherAndTimeOfDayComponentDesc", void *> k_Career_WeatherAndTimeOfDayComponentDesc{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeBlockNameDefault"> site_Career_ApplyUpgradeBlockNameDefault{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeBlockNameJoin"> site_Career_ApplyUpgradeBlockNameJoin{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeKeyCall"> site_Career_ApplyUpgradeKeyCall{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeKeyFilter"> site_Career_ApplyUpgradeKeyFilter{}; // L2 experimental
inline constexpr site<"site_Career_DeserialiseUpgradesEnd"> site_Career_DeserialiseUpgradesEnd{}; // L2 experimental
inline constexpr site<"site_Career_LoadUpgradeRows_DeptPath"> site_Career_LoadUpgradeRows_DeptPath{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridFirstTab"> site_Career_RnDBoxGridFirstTab{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridNameClamp"> site_Career_RnDBoxGridNameClamp{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridNameIndex"> site_Career_RnDBoxGridNameIndex{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridNameRead"> site_Career_RnDBoxGridNameRead{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtImage0"> site_Career_RnDCardArtImage0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtImage1"> site_Career_RnDCardArtImage1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtImage2"> site_Career_RnDCardArtImage2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtMask0"> site_Career_RnDCardArtMask0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtMask1"> site_Career_RnDCardArtMask1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtMask2"> site_Career_RnDCardArtMask2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick0"> site_Career_RnDCardColourPick0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick1"> site_Career_RnDCardColourPick1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick2"> site_Career_RnDCardColourPick2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick3"> site_Career_RnDCardColourPick3{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick4"> site_Career_RnDCardColourPick4{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardDeptIcon0"> site_Career_RnDCardDeptIcon0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardDeptIcon1"> site_Career_RnDCardDeptIcon1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardDeptIcon2"> site_Career_RnDCardDeptIcon2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCompareDept"> site_Career_RnDCompareDept{}; // L2 experimental
inline constexpr site<"site_Career_RnDContainerOpenMaint"> site_Career_RnDContainerOpenMaint{}; // L2 experimental
inline constexpr site<"site_Career_RnDContainerOpenTail"> site_Career_RnDContainerOpenTail{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptBoxGridEntry"> site_Career_RnDDeptBoxGridEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptBoxGridInject"> site_Career_RnDDeptBoxGridInject{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptBoxLayoutProbe"> site_Career_RnDDeptBoxLayoutProbe{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptPagePreQuery"> site_Career_RnDDeptPagePreQuery{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptPageQueryDone"> site_Career_RnDDeptPageQueryDone{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptPageTypeLoop"> site_Career_RnDDeptPageTypeLoop{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowCategoryIcon"> site_Career_RnDDeptRowCategoryIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeIcon"> site_Career_RnDDeptRowTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeIconJoin"> site_Career_RnDDeptRowTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeLabel"> site_Career_RnDDeptRowTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeLabelJoin"> site_Career_RnDDeptRowTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressColourStore"> site_Career_RnDInProgressColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressPartIcon"> site_Career_RnDInProgressPartIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressStrip"> site_Career_RnDInProgressStrip{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeIcon"> site_Career_RnDInProgressTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeIconJoin"> site_Career_RnDInProgressTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeLabel"> site_Career_RnDInProgressTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeLabelJoin"> site_Career_RnDInProgressTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDListRowColourStore"> site_Career_RnDListRowColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDListRowDeptIcon"> site_Career_RnDListRowDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDListRowLightColourStore"> site_Career_RnDListRowLightColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDPageModifierList"> site_Career_RnDPageModifierList{}; // L2 experimental
inline constexpr site<"site_Career_RnDPanelModifierList"> site_Career_RnDPanelModifierList{}; // L2 experimental
inline constexpr site<"site_Career_RnDPanelResearchLimit"> site_Career_RnDPanelResearchLimit{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfChildName"> site_Career_RnDPerfChildName{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfDeptColour"> site_Career_RnDPerfDeptColour{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfDeptLoopEnd"> site_Career_RnDPerfDeptLoopEnd{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfDeptName"> site_Career_RnDPerfDeptName{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfGotoName"> site_Career_RnDPerfGotoName{}; // L2 experimental
inline constexpr call<"site_Career_RnDPerfViewModelSlotDtor"> site_Career_RnDPerfViewModelSlotDtor{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerksTitleDeptIcon"> site_Career_RnDPerksTitleDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieBufSize"> site_Career_RnDPieBufSize{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieCtorRet"> site_Career_RnDPieCtorRet{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieFocusJoin"> site_Career_RnDPieFocusJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieFocusNameCmp"> site_Career_RnDPieFocusNameCmp{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieFocusRestore"> site_Career_RnDPieFocusRestore{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieMoveAssignSlotDtor"> site_Career_RnDPieMoveAssignSlotDtor{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieMovedFromSlotDtor"> site_Career_RnDPieMovedFromSlotDtor{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateDeptGuard"> site_Career_RnDPiePopulateDeptGuard{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateGate"> site_Career_RnDPiePopulateGate{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateNextCode"> site_Career_RnDPiePopulateNextCode{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateSlotCap"> site_Career_RnDPiePopulateSlotCap{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieSliceDefaultColour"> site_Career_RnDPieSliceDefaultColour{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieSliceDescFormat"> site_Career_RnDPieSliceDescFormat{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieSliceIconArgs"> site_Career_RnDPieSliceIconArgs{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieSliceNameJoin"> site_Career_RnDPieSliceNameJoin{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieTeardownSlotDtor"> site_Career_RnDPieTeardownSlotDtor{}; // L2 experimental
inline constexpr site<"site_Career_RnDPopupCategoryIcon"> site_Career_RnDPopupCategoryIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDPopupColourStore"> site_Career_RnDPopupColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDPotentialPieEntry"> site_Career_RnDPotentialPieEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseBackOpen"> site_Career_RnDPurchaseBackOpen{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseOptionsEntry"> site_Career_RnDPurchaseOptionsEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchasePanelTypeLabel"> site_Career_RnDPurchasePanelTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchasePanelTypeLabelJoin"> site_Career_RnDPurchasePanelTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseRequestEntry"> site_Career_RnDPurchaseRequestEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseScreenTypeLabel"> site_Career_RnDPurchaseScreenTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseScreenTypeLabelJoin"> site_Career_RnDPurchaseScreenTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchFocusNameCmp"> site_Career_RnDResearchFocusNameCmp{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowCategoryIcon"> site_Career_RnDResearchRowCategoryIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeIcon"> site_Career_RnDResearchRowTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeIconJoin"> site_Career_RnDResearchRowTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeLabel"> site_Career_RnDResearchRowTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeLabelJoin"> site_Career_RnDResearchRowTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchTypeLoop"> site_Career_RnDResearchTypeLoop{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeIcon"> site_Career_RnDResultTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeIconJoin"> site_Career_RnDResultTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeLabel"> site_Career_RnDResultTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeLabelJoin"> site_Career_RnDResultTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountBody"> site_Career_RnDTileCountBody{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountExit"> site_Career_RnDTileCountExit{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountGuards"> site_Career_RnDTileCountGuards{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountIndexGuard"> site_Career_RnDTileCountIndexGuard{}; // L2 experimental
inline constexpr site<"site_Career_RnDTodoTypeLabel"> site_Career_RnDTodoTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDTodoTypeLabelJoin"> site_Career_RnDTodoTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDTreeRowCountProbe"> site_Career_RnDTreeRowCountProbe{}; // L2 experimental
inline constexpr site<"site_Career_RnDUpgradeInfoTypeLabel"> site_Career_RnDUpgradeInfoTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDUpgradeInfoTypeLabelJoin"> site_Career_RnDUpgradeInfoTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDUpgradeListDeptIcon"> site_Career_RnDUpgradeListDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_ScheduleComponent_TrackListEntryDone"> site_Career_ScheduleComponent_TrackListEntryDone{}; // L2 experimental
inline constexpr site<"site_Career_ScheduleComponent_TrackListNextRow"> site_Career_ScheduleComponent_TrackListNextRow{}; // L2 experimental
inline constexpr site<"site_Career_ScheduleComponent_TrackListPush"> site_Career_ScheduleComponent_TrackListPush{}; // L2 experimental
inline constexpr site<"site_Career_ScheduleComponent_TrackListSave"> site_Career_ScheduleComponent_TrackListSave{}; // L2 experimental
inline constexpr site<"site_Career_StandingsWeekendTrackDone"> site_Career_StandingsWeekendTrackDone{}; // L2 experimental
inline constexpr site<"site_Career_StandingsWeekendTrackLoad"> site_Career_StandingsWeekendTrackLoad{}; // L2 experimental
inline constexpr site<"site_Career_TeamDeptEnumStore"> site_Career_TeamDeptEnumStore{}; // L2 experimental
inline constexpr site<"site_Career_TeamHqIsMyTeamResult"> site_Career_TeamHqIsMyTeamResult{}; // L2 experimental
inline constexpr site<"site_Career_TeamHqRnDPopupTypeIcon"> site_Career_TeamHqRnDPopupTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_TeamHqRnDPopupTypeIconJoin"> site_Career_TeamHqRnDPopupTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_TeleFailedTypeName"> site_Career_TeleFailedTypeName{}; // L2 experimental
inline constexpr site<"site_Career_TeleFailedTypeNameJoin"> site_Career_TeleFailedTypeNameJoin{}; // L2 experimental
inline constexpr site<"site_Career_TeleRecordTypeName"> site_Career_TeleRecordTypeName{}; // L2 experimental
inline constexpr call<"site_Career_TeleRecordTypeNameCall"> site_Career_TeleRecordTypeNameCall{}; // L2 experimental
inline constexpr site<"site_Career_TeleResearchTypeName"> site_Career_TeleResearchTypeName{}; // L2 experimental
inline constexpr call<"site_Career_TeleResearchTypeNameCall"> site_Career_TeleResearchTypeNameCall{}; // L2 experimental
inline constexpr site<"site_Career_Unk141814320TypeLabel"> site_Career_Unk141814320TypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_Unk141814320TypeLabelJoin"> site_Career_Unk141814320TypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_Unk1418169f0TypeLabel"> site_Career_Unk1418169f0TypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_Unk1418169f0TypeLabelJoin"> site_Career_Unk1418169f0TypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeAutoFitBody"> site_Career_UpgradeAutoFitBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeAutoFitDeptGate"> site_Career_UpgradeAutoFitDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeBlockReplayEntry"> site_Career_UpgradeBlockReplayEntry{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeCardDeptIcon"> site_Career_UpgradeCardDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeCardTypeLabel"> site_Career_UpgradeCardTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeCardTypeLabelJoin"> site_Career_UpgradeCardTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeGraphHubTest"> site_Career_UpgradeGraphHubTest{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeGraphRegularNode"> site_Career_UpgradeGraphRegularNode{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeInitFakeDeptBranch"> site_Career_UpgradeInitFakeDeptBranch{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeInitNormal"> site_Career_UpgradeInitNormal{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeMetaTypeJoin"> site_Career_UpgradeMetaTypeJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradePushStateBody"> site_Career_UpgradePushStateBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradePushStateDeptGate"> site_Career_UpgradePushStateDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeRowTypeJoin"> site_Career_UpgradeRowTypeJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetEnabledBody"> site_Career_UpgradeSetEnabledBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetEnabledDeptGate"> site_Career_UpgradeSetEnabledDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetFittedBody"> site_Career_UpgradeSetFittedBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetFittedDeptGate"> site_Career_UpgradeSetFittedDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetLockedBody"> site_Career_UpgradeSetLockedBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetLockedDeptGate"> site_Career_UpgradeSetLockedDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_VehicleFillOrderEnd"> site_Career_VehicleFillOrderEnd{}; // L2 experimental
inline constexpr site<"site_Career_VehicleFillOrderStart"> site_Career_VehicleFillOrderStart{}; // L2 experimental
inline constexpr site<"site_Career_WeekendObjectiveNext"> site_Career_WeekendObjectiveNext{}; // L2 experimental
inline constexpr site<"site_Career_WeekendObjectiveType"> site_Career_WeekendObjectiveType{}; // L2 experimental
inline constexpr site<"site_Career_WeekendsTrackRecordLoad"> site_Career_WeekendsTrackRecordLoad{}; // L2 experimental
inline constexpr vtable<"vt_Career_BankTransactionMessage", 2> vt_Career_BankTransactionMessage{}; // L2 experimental
inline constexpr vtable<"vt_Career_Championship", 12> vt_Career_Championship{}; // L2 experimental
inline constexpr vtable<"vt_Career_FacilityNode", 11> vt_Career_FacilityNode{}; // L2 experimental
inline constexpr vtable<"vt_Career_MessageDriverContractChanged", 2> vt_Career_MessageDriverContractChanged{}; // L2 experimental
inline constexpr vtable<"vt_Career_MessageDriverMoveConfirmed", 2> vt_Career_MessageDriverMoveConfirmed{}; // L2 experimental
inline constexpr vtable<"vt_Career_MessageDriverMovesFinalised", 2> vt_Career_MessageDriverMovesFinalised{}; // L2 experimental
inline constexpr vtable<"vt_Career_MessageDriverMovesFinalisedRecognition", 2> vt_Career_MessageDriverMovesFinalisedRecognition{}; // L2 experimental
inline constexpr vtable<"vt_Career_MessageDriverMovesFinalisedUpgrades", 2> vt_Career_MessageDriverMovesFinalisedUpgrades{}; // L2 experimental
inline constexpr vtable<"vt_Career_RenownAwardedMessage", 2> vt_Career_RenownAwardedMessage{}; // L2 experimental
inline constexpr vtable<"vt_Career_RnDDeptCard", 0> vt_Career_RnDDeptCard{}; // L2 experimental
inline constexpr vtable<"vt_Career_Season", 7> vt_Career_Season{}; // L2 experimental
inline constexpr vtable<"vt_Career_SponsorLoyaltyXpMessage", 2> vt_Career_SponsorLoyaltyXpMessage{}; // L2 experimental
inline constexpr vtable<"vt_Career_Weekend", 7> vt_Career_Weekend{}; // L2 experimental

}
