--
-- PostgreSQL database dump
--


-- Dumped from database version 18.6
-- Dumped by pg_dump version 18.6

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: event; Type: TABLE DATA; Schema: explore; Owner: -
--

INSERT INTO explore.event VALUES ('edited', 'edited here', NULL, NULL, NULL, NULL, NULL, false, false, false, false, NULL, NULL, NULL, NULL, NULL, NULL, '{}', '2026-11-03 19:13:44.308703', 'upstream');
INSERT INTO explore.event VALUES ('plain', 'plain', NULL, NULL, NULL, NULL, NULL, false, false, false, false, NULL, NULL, NULL, NULL, NULL, NULL, '{}', '2026-11-03 19:13:44.308703', 'upstream');
INSERT INTO explore.event VALUES ('rejected-here', 'rejected-here', NULL, NULL, NULL, NULL, NULL, false, true, false, false, NULL, NULL, NULL, NULL, NULL, NULL, '{"approved": false, "rejected": true, "highlighted": true, "rejected_by": "0xd", "rejection_reason": "spam"}', '2026-11-03 19:13:44.308972', 'upstream');
INSERT INTO explore.event VALUES ('deleted-here', 'deleted-here', NULL, NULL, NULL, NULL, NULL, false, false, false, true, NULL, NULL, NULL, NULL, NULL, NULL, '{"approved": true, "deleted_by": "0xd", "deleted_reason": "duplicate", "deleted_by_admin": true}', '2026-11-03 19:13:44.308972', 'upstream');
INSERT INTO explore.event VALUES ('featured-here', 'featured-here', NULL, NULL, NULL, NULL, NULL, false, true, false, true, NULL, NULL, NULL, NULL, NULL, NULL, '{"approved": true, "highlighted": true}', '2026-11-03 19:13:44.309307', 'upstream');
INSERT INTO explore.event VALUES ('created', 'created', NULL, NULL, NULL, NULL, NULL, false, false, false, false, NULL, NULL, NULL, NULL, NULL, NULL, '{}', '2026-11-03 20:13:44.306132', 'local');
INSERT INTO explore.event VALUES ('moderated', 'moderated', NULL, NULL, NULL, NULL, NULL, false, false, false, false, NULL, NULL, NULL, NULL, NULL, NULL, '{}', '2026-11-03 20:13:44.307744', 'local');


--
-- Data for Name: event_moderation; Type: TABLE DATA; Schema: explore; Owner: -
--

INSERT INTO explore.event_moderation VALUES ('rejected-here', false, true, true, NULL, NULL, NULL, '0xd', 'spam', NULL, NULL, '0xd', '2026-11-03 20:13:44.308972');
INSERT INTO explore.event_moderation VALUES ('deleted-here', NULL, NULL, NULL, NULL, true, NULL, NULL, NULL, '0xd', 'duplicate', '0xd', '2026-11-03 20:13:44.308972');
INSERT INTO explore.event_moderation VALUES ('featured-here', NULL, NULL, true, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '0xd', '2026-11-03 20:13:44.309656');


--
-- Data for Name: events_local; Type: TABLE DATA; Schema: explore; Owner: -
--

INSERT INTO explore.events_local VALUES ('created', '0xc', '{"create": {}}', '2026-11-03 20:13:44.306132', '2026-11-03 20:13:44.306132');
INSERT INTO explore.events_local VALUES ('moderated', '0xc', '{"moderation": {}}', '2026-11-03 20:13:44.307744', '2026-11-03 21:13:44.307744');
INSERT INTO explore.events_local VALUES ('edited', '0xc', '{"edit": {}}', '2026-11-03 20:13:44.308703', '2026-11-03 20:13:44.308703');
INSERT INTO explore.events_local VALUES ('rejected-here', '0xd', '{"moderation": {"action": "reject", "highlighted": true}}', '2026-11-03 20:13:44.308972', '2026-11-03 20:13:44.308972');
INSERT INTO explore.events_local VALUES ('deleted-here', '0xd', '{"delete": {}}', '2026-11-03 20:13:44.308972', '2026-11-03 20:13:44.308972');
INSERT INTO explore.events_local VALUES ('featured-here', '0xd', '{"moderation": {"name": null, "action": "feature", "reason": null, "approved": null, "rejected": null, "trending": null, "description": null, "highlighted": null}}', '2026-11-03 20:13:44.309656', '2026-11-03 20:13:44.309656');


--
-- Data for Name: place; Type: TABLE DATA; Schema: explore; Owner: -
--

INSERT INTO explore.place VALUES ('mirrored', '2,2', NULL, NULL, NULL, NULL, '{}', 0, 0, 0, NULL, false, false, '{}', '2026-11-03 20:13:44.306132', DEFAULT, DEFAULT, DEFAULT, DEFAULT, 'upstream');
INSERT INTO explore.place VALUES ('derived', '1,1', NULL, NULL, NULL, NULL, '{}', 0, 0, 0, NULL, false, false, '{"local": true}', '2026-11-03 20:13:44.306132', DEFAULT, DEFAULT, DEFAULT, DEFAULT, 'local');


--
-- Data for Name: communities; Type: TABLE DATA; Schema: social; Owner: -
--

INSERT INTO social.communities VALUES ('64cffcd8-ada9-4a57-ae4e-90b203929e65', 'here', '', '0xcccccccccccccccccccccccccccccccccccccccc', false, true, false, false, 0, false, NULL, NULL, NULL, '2026-11-03 20:13:44.310036', '2026-11-03 20:13:44.310036', 'local', NULL, NULL, NULL, 0, 0, 0, 0, 0, '2026-11-03 20:13:44.335646');
INSERT INTO social.communities VALUES ('dc86ca2b-7f80-469f-b3f6-8e7222790af6', 'there', '', '0xcccccccccccccccccccccccccccccccccccccccc', false, true, false, false, 0, false, NULL, NULL, NULL, '2026-11-03 20:13:44.310036', '2026-11-03 20:13:44.310036', 'upstream', NULL, NULL, NULL, 0, 0, 0, 0, 0, '2026-11-03 20:13:44.335646');


--
-- Data for Name: community_members; Type: TABLE DATA; Schema: social; Owner: -
--

INSERT INTO social.community_members VALUES ('64cffcd8-ada9-4a57-ae4e-90b203929e65', '0xcccccccccccccccccccccccccccccccccccccccc', 'owner', '2026-11-03 20:13:44.310647');


--
-- PostgreSQL database dump complete
--
