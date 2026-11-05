#!/bin/sh
# Runs on the host, with Zed quit, after Zed has created a new container for this checkout: points
# Zed's agent threads of this checkout's old dev containers at the current one, and removes those
# containers' project groups from Zed's windows. Zed files both under the container's id, which
# changes whenever the container is created anew; it then neither shows nor opens the threads, and
# keeps a group that does not open (https://github.com/zed-industries/zed/issues/56576). Remove this
# script once Zed gives dev containers a stable identity.
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

# Whether a container id of this checkout is an old one. Zed looks a thread up by the full id, but
# older threads may hold the short one: a short form of the current id is old too, while any other
# container that still exists is not (another checkout).
is_old() {
    if [ "$1" = "$new" ]; then
        return 1
    fi
    case $new in
        "$1"*) return 0 ;;
    esac
    ! docker container inspect "$1" >/dev/null 2>&1
}

filter="json_extract(remote_connection, '\$.Docker.name') = $(sql_quote "$name")
    AND json_extract(remote_connection, '\$.Docker.remote_user') = $(sql_quote "$user")
    AND folder_paths LIKE $(sql_quote "%$workspace%")"

cp "$db" "$db.bak"
count=0
for old in $(sqlite3 "$db" "SELECT DISTINCT json_extract(remote_connection, '\$.Docker.container_id')
    FROM sidebar_threads WHERE $filter;"); do
    if ! is_old "$old"; then
        continue
    fi
    rows=$(sqlite3 "$db" "UPDATE sidebar_threads
        SET remote_connection = json_set(remote_connection, '\$.Docker.container_id', '$new')
                WHERE $filter AND json_extract(remote_connection, '\$.Docker.container_id') = '$old';
        SELECT changes();")
    echo "$old -> $new: $rows threads"
    count=$((count + rows))
done
echo "Rebound $count threads (backup: $db.bak). Archived threads stay hidden."

# The threads have left these groups, so removing them loses nothing. A window holds its groups as
# JSON; the inner json() keeps the list an array rather than storing it as a string.
docker_id="json_extract(g.value, '\$.location.Remote.Docker.container_id')"
group_filter="json_extract(g.value, '\$.location.Remote.Docker.name') = $(sql_quote "$name")
    AND json_extract(g.value, '\$.location.Remote.Docker.remote_user') = $(sql_quote "$user")
    AND json_extract(g.value, '\$.path_list.paths') LIKE $(sql_quote "%$workspace%")"
for old in $(sqlite3 "$db" "SELECT DISTINCT $docker_id
    FROM scoped_kv_store s, json_each(s.value, '\$.project_groups') g
    WHERE s.namespace = 'multi_workspace_state' AND $group_filter;"); do
    if ! is_old "$old"; then
        continue
    fi
    rows=$(sqlite3 "$db" "UPDATE scoped_kv_store
        SET value = json_set(value, '\$.project_groups', json((
            SELECT json_group_array(json(g.value))
            FROM json_each(scoped_kv_store.value, '\$.project_groups') g
            WHERE coalesce($docker_id, '') <> '$old')))
        WHERE namespace = 'multi_workspace_state' AND value LIKE '%$old%';
        SELECT changes();")
    echo "Removed the project group of $old from $rows windows"
done
