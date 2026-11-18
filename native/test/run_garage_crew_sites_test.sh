#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
status=0
for site in $(grep -ohE '(SITE|SETUP_SITE|SETUP_SKIP|SCENE_SITE|SCENE_TEST) = "site_[A-Za-z0-9_]+"' ../src/core/garage_crew.cpp | grep -oE 'site_[A-Za-z0-9_]+'); do
    if grep -q "site<\"$site\">" ../include/swofty/game/game.hpp; then
        echo "ok  : $site is exported"
    else
        echo "FAIL: $site is not exported; the garage crew guard turns off without it"
        status=1
    fi
done
exit $status
