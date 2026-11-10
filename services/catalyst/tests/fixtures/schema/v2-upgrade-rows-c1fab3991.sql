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
-- Data for Name: purchases; Type: TABLE DATA; Schema: credits; Owner: -
--

INSERT INTO credits.purchases OVERRIDING SYSTEM VALUE VALUES (1, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'credits_usd_5', 50, 500, 'usd', 'card', 'paid', NULL, NULL, NULL, NULL, 0, 0, NULL, '2026-11-03 20:02:47.93961', '2026-11-03 20:02:47.93961', 1, NULL);


--
-- Data for Name: event_profile_settings; Type: TABLE DATA; Schema: explore; Owner: -
--

INSERT INTO explore.event_profile_settings VALUES ('0xlegacy', NULL, false, true, false, false, '["edit_any_event"]', '2026-11-03 20:02:47.942428', '2026-11-03 20:02:47.942428', false);


--
-- Data for Name: worlds; Type: TABLE DATA; Schema: worlds; Owner: -
--

INSERT INTO worlds.worlds VALUES ('upgrade.dcl.eth', '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'unrestricted', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, false, true, NULL, NULL, 0, NULL, NULL, '2026-11-03 20:02:47.94033', '2026-11-03 20:02:47.94033');


--
-- Name: purchases_id_seq; Type: SEQUENCE SET; Schema: credits; Owner: -
--

SELECT pg_catalog.setval('credits.purchases_id_seq', 1, true);


--
-- PostgreSQL database dump complete
--
