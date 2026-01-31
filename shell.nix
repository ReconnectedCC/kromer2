{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    postgresql
    cargo
    rustc
  ];

  shellHook = ''
    set -e

    export PGDATA="$PWD/.postgres"
    export PGHOST="$PGDATA"
    export PGPORT=5432
    export PGUSER=root
    export PGPASSWORD=root
    export DATABASE_URL="postgresql://root:root@localhost:5432/postgres"

    echo "Using local postgres at $PGDATA"

    if [ ! -d "$PGDATA" ]; then
      echo "Initializing database..."
      initdb -D "$PGDATA" --no-locale
    fi

    if ! pg_ctl -D "$PGDATA" status >/dev/null 2>&1; then
      echo "Starting postgres..."
      pg_ctl -D "$PGDATA" -l "$PGDATA/logfile" -o "-k $PGDATA" start
    fi

    createuser root -s 2>/dev/null || true
    psql -d postgres -c "ALTER USER root WITH PASSWORD 'root';" >/dev/null 2>&1 || true

    echo ""
    echo "Postgres ready"
    echo "DATABASE_URL=$DATABASE_URL"
    echo ""

    cleanup() {
      echo "Stopping postgres..."
      pg_ctl -D "$PGDATA" stop >/dev/null 2>&1 || true
    }

    trap cleanup EXIT
  '';
}
