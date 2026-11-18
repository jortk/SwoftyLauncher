1.0: first version (launcher 1.6.0).
1.1: rules.car_profile names a car.profile record (launcher 1.7.0).
1.2: leaderboard names the Time Trial leaderboard of the entrant list: its own by default (launcher 1.7.20).
1.3: virtual_list keeps the entrant list out of the stat ids and setup mode slots; the core reads it into swofty_virtual_list when its box is pressed (launcher 1.9.10).
1.4: rules.tyre_warmers turns tyre warmers on or off for the box's races; the era picker writes it into the swofty_f1 setup mode when the box is pressed.
1.5: rules.refuel_stint_kg, rules.quali_fuel_kg and rules.race_start_fuel_from_quali; the core applies rules.refuelling and rules.race_fuel_kg in the box's races (start fuel, a refill at every pit stop) and the parc ferme fuel rule; rules.race_tyre_change (false: race pit stops keep the tyres, damage excepted).
1.5: rules.race_time_limit_s and rules.weekend_mode set the box race time limit and its weekend (practice, qualifying, race groups) from a helper setup mode; the era picker swaps both into the swofty_f1 setup mode when the box is pressed.
1.5, core change (no schema change): the core sets available to true on the game_setup_modes row rules.weekend_mode names at boot, so the game parses the helper into its setup mode store (it marks only modes a game mode lists).
1.5: rules.safety_car sets the game's safety car option (off, reduced, standard, increased) for the box's races; the era picker sets it when the box is pressed and puts the player's value back when another box is pressed. rules.formation_lap (bool), rules.penalties (off, corner_cutting_only, esports, on) and rules.corner_cutting (regular, strict) set the game's formation lap, rules and penalties and corner cutting options the same way.
1.5: rules.grid sets how the box's race grid forms: qualifying scoring (best_lap, best_of, aggregate), the 107 % rule, and a row pattern (design only); the core re-ranks the weekend's qualifying classification in that box.
1.5: rules.qualifying_tyres names the dry class options a box may use only in qualifying; the core refuses them in race sessions after the box is pressed.
1.6: rules.mandatory_tyre_change turns the two-compound rule off or on for the box's races; the era picker writes swofty_f1's tyre regulation requirement flags when the box is pressed.
