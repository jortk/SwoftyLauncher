1.0: first version (launcher 1.6.5), experimental.
1.0, core change (no schema change): pit.speed_limit is applied per box (TrackDataTrack +0x21c) like fuel.
1.0, core change (no schema change): laps is applied per box in a Grand Prix box (the race's laps at full race length, written into the number_of_laps value TrackDataTrack +0xe0 points at; the race length option scales from it), put back when the box is left.
