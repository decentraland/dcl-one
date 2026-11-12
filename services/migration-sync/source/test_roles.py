#!/usr/bin/env python3
"""Disposable local PostgreSQL smoke test for grants.sql and preflight.sql."""
import os
from pathlib import Path
import subprocess
from urllib.parse import urlsplit, urlunsplit
import uuid

root = Path(__file__).resolve().parent
url = os.environ['MIGRATION_SYNC_TEST_DATABASE_URL']
parsed = urlsplit(url)
if parsed.hostname not in ('127.0.0.1', 'localhost', '::1'):
    raise SystemExit('This destructive fixture test requires a local disposable PostgreSQL server')
suffix = uuid.uuid4().hex[:12]
database, writer, reader = (f'sync_roles_{kind}_{suffix}' for kind in ('db', 'writer', 'reader'))
test_url = urlunsplit(parsed._replace(path='/' + database))


def psql(sql=None, *, source=None, file=None, admin=False, succeeds=True):
    command = ['psql', '-X', '-q', '-v', 'ON_ERROR_STOP=1', '-d', url if admin else test_url]
    if source:
        command += ['-v', f'source={source}', '-v', f'writer_role={writer}', '-v', f'reader_role={reader}']
    if file:
        command += ['-f', str(root / file)]
    result = subprocess.run(command, input=sql, text=True, capture_output=True)
    if (result.returncode == 0) != succeeds:
        raise AssertionError(result.stderr or result.stdout or 'Unexpected psql exit status')


try:
    psql(f'CREATE ROLE {writer}; CREATE ROLE {reader};', admin=True)
    psql(f'CREATE DATABASE {database};', admin=True)
    psql('''
CREATE TABLE public.places(id char(36) PRIMARY KEY, world boolean DEFAULT false, disabled boolean DEFAULT false, favorites integer DEFAULT 0);
CREATE TABLE public.user_favorites("user" varchar(42), entity_id char(36), user_activity integer, created_at timestamptz);
CREATE TABLE public.user_likes("user" varchar(42), entity_id char(36), "like" boolean, user_activity integer, created_at timestamptz, updated_at timestamptz);
CREATE TABLE public.schedule(id uuid PRIMARY KEY,name varchar(50),description text,image varchar(255),theme varchar(25),background varchar(30)[],active boolean,active_since timestamptz,active_until timestamptz,created_at timestamptz,updated_at timestamptz);
CREATE TABLE public.social_settings(address varchar(42) PRIMARY KEY,private_messages_privacy text DEFAULT 'only_friends',blocked_users_messages_visibility text DEFAULT 'show_messages',show_situation_reactions text DEFAULT 'show');
''')
    psql(f'GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {writer};')
    place = '11111111-1111-4111-8111-111111111111'
    wallet = '0x' + '1' * 40
    for source, adapter, schema, scope in [
        ('places', 'places.sql', 'migration_capture', place),
        ('events', 'events_schedules.sql', 'migration_capture_events', place),
        ('social-settings', 'social-settings.sql', 'migration_capture_social_settings', wallet),
    ]:
        psql(file=adapter)
        psql(f"INSERT INTO {schema}.scope VALUES('{scope}');")
        psql(source=source, file='preflight.sql', succeeds=False)
        psql(source=source, file='grants.sql')
        psql(source=source, file='preflight.sql')
    psql(f'''SET ROLE {writer};
INSERT INTO public.places(id) VALUES('{place}');
INSERT INTO public.user_favorites VALUES('{wallet}','{place}',1,now());
INSERT INTO public.user_likes VALUES('{wallet}','{place}',true,1,now(),now());
INSERT INTO public.schedule VALUES('{place}','Example',NULL,NULL,NULL,ARRAY['red'],true,now(),now(),now(),now());
INSERT INTO public.social_settings(address) VALUES('{wallet}');
UPDATE public.places SET disabled=true WHERE id='{place}';
UPDATE public.schedule SET name='Changed';
UPDATE public.social_settings SET show_situation_reactions='hide';
DELETE FROM public.user_favorites;
DELETE FROM public.user_likes;
DELETE FROM public.places;
DELETE FROM public.schedule;
DELETE FROM public.social_settings;
''')
    for schema in ('migration_capture','migration_capture_events','migration_capture_social_settings'):
        psql(f"SET ROLE {reader}; SELECT * FROM {schema}.control; SELECT * FROM {schema}.changes;")
        psql(f"SET ROLE {reader}; UPDATE {schema}.control SET position=0;", succeeds=False)
        psql(f"SET ROLE {reader}; SELECT {schema}.ensure_transaction();", succeeds=False)
    psql(f'GRANT {writer} TO {reader} WITH INHERIT FALSE;')
    psql(source='events', file='preflight.sql', succeeds=False)
    psql(f'REVOKE {writer} FROM {reader};')
    psql(f'ALTER SCHEMA migration_capture_events OWNER TO {reader};')
    psql(source='events', file='preflight.sql', succeeds=False)
    psql('ALTER SCHEMA migration_capture_events OWNER TO CURRENT_USER;')
    psql(f'REVOKE CREATE ON SCHEMA migration_capture_events FROM {reader};')
    psql(source='events', file='grants.sql')
    psql(source='events', file='preflight.sql')
    psql(f'GRANT UPDATE(position) ON migration_capture_events.control TO {reader};')
    psql(source='events', file='preflight.sql', succeeds=False)
    psql(f'REVOKE UPDATE(position) ON migration_capture_events.control FROM {reader};')
    psql(source='events', file='preflight.sql')
    psql(f'REVOKE INSERT ON migration_capture_events.transactions FROM {writer};')
    psql(source='events', file='preflight.sql', succeeds=False)
    psql(source='events', file='grants.sql')
    psql('ALTER TABLE public.schedule DISABLE TRIGGER migration_capture_row;')
    psql(source='events', file='preflight.sql', succeeds=False)
    print('Three adapters: writer CRUD, reader isolation and negative preflight tests passed')
finally:
    psql(f'DROP DATABASE IF EXISTS {database};', admin=True)
    psql(f'DROP ROLE IF EXISTS {writer}; DROP ROLE IF EXISTS {reader};', admin=True)
