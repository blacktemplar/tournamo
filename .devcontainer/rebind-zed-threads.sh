#!/bin/sh
# Runs on the host, with Zed quit, after Zed has created a new container for this checkout: points
# Zed's agent threads of this checkout's old dev containers at the current one. Zed files a thread
# under the container's id, which changes whenever the container is created anew, and then neither
# shows nor opens it (https://github.com/zed-industries/zed/issues/56576). Remove this script once
# Zed gives dev containers a stable identity.
set -eu

cd "$(dirname "$0")/.."
workspace=$(pwd -P)
# as initialize.sh names the compose project
hash=$(printf '%s' "$workspace" | sha256sum | cut -c1-8)
db=${ZED_DB:-${XDG_DATA_HOME:-$HOME/.local/share}/zed/db/0-stable/db.sqlite}
# what Zed keys the connection by besides the id: `name` and `remoteUser` of devcontainer.json
name=Tournamo
user=dev

if pgrep -x zed >/dev/null || pgrep -x zed-editor >/dev/null; then
    echo "Quit Zed first: it keeps the threads in memory and would write them back." >&2
    exit 1
fi
new=$(docker compose -p "tournamo-$hash" ps -aq --no-trunc dev)
if [ -z "$new" ]; then
    echo "No container of tournamo-$hash yet: open the project in Zed once, then quit it." >&2
    exit 1
fi

sql_quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/''/g")"; }
filter="json_extract(remote_connection, '\$.Docker.name') = $(sql_quote "$name")
    AND json_extract(remote_connection, '\$.Docker.remote_user') = $(sql_quote "$user")
    AND folder_paths LIKE $(sql_quote "%$workspace%")"

cp "$db" "$db.bak"
count=0
for old in $(sqlite3 "$db" "SELECT DISTINCT json_extract(remote_connection, '\$.Docker.container_id')
    FROM sidebar_threads WHERE $filter;"); do
    # a container that still exists keeps its threads (another checkout, or the current one)
    if docker container inspect "$old" >/dev/null 2>&1; then
        continue
    fi
    # Zed may keep the short id or the full one: keep the old id's form
    current=$(printf '%s' "$new" | cut -c1-${#old})
    rows=$(sqlite3 "$db" "UPDATE sidebar_threads
        SET remote_connection = json_set(remote_connection, '\$.Docker.container_id', '$current')
        WHERE $filter AND json_extract(remote_connection, '\$.Docker.container_id') = '$old';
        SELECT changes();")
    echo "$old -> $current: $rows threads"
    count=$((count + rows))
done
echo "Rebound $count threads (backup: $db.bak). Archived threads stay hidden."
