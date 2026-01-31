{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    postgresql
    cargo
    rustc
    rustfmt
  ];

  shellHook = ''
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

    psql -d postgres <<'SQL'
    DO $$
    BEGIN
      IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'root') THEN
        CREATE ROLE root WITH LOGIN SUPERUSER PASSWORD 'root';
      END IF;
    END
    $$;
    SQL

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
