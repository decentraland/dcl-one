-- Install with the reviewed hybrid adapter, never as a generic catalogue trigger.
-- Only the explicitly opted-in source POI job calls this function.
CREATE FUNCTION migration_capture.apply_poi_membership(entity_id uuid, poi boolean)
RETURNS jsonb LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
DECLARE current_categories text[]; next_categories text[]; normalized text[]; scoped boolean; result jsonb;
BEGIN
    IF entity_id IS NULL OR entity_id='00000000-0000-0000-0000-000000000000'::uuid OR poi IS NULL THEN
        RAISE EXCEPTION 'invalid POI membership outcome';
    END IF;
    PERFORM pg_advisory_xact_lock(179321,1);
    SELECT p.categories INTO current_categories FROM public.places p
      WHERE p.id::text=apply_poi_membership.entity_id::text AND NOT p.world FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'POI outcome requires an existing Genesis place'; END IF;
    IF current_categories IS NULL OR array_position(current_categories,NULL) IS NOT NULL THEN
        RAISE EXCEPTION 'invalid source categories';
    END IF;
    SELECT EXISTS(SELECT 1 FROM migration_capture.scope s WHERE s.entity_id=apply_poi_membership.entity_id) INTO scoped;
    SELECT COALESCE(array_agg(pc.category_id::text),ARRAY[]::text[]) INTO normalized
      FROM public.place_categories pc WHERE pc.place_id::text=apply_poi_membership.entity_id::text;
    -- A job that would repair unrelated category drift is not a POI-only result.
    IF scoped AND (ARRAY(SELECT DISTINCT c FROM unnest(current_categories) c WHERE c<>'poi' ORDER BY c)
      IS DISTINCT FROM ARRAY(SELECT DISTINCT c FROM unnest(normalized) c WHERE c<>'poi' ORDER BY c)) THEN
        RAISE EXCEPTION 'POI job would change unrelated categories';
    END IF;
    next_categories:=array_remove(current_categories,'poi');
    IF poi THEN
        next_categories:=array_append(next_categories,'poi');
        INSERT INTO public.place_categories(place_id,category_id)
          VALUES(entity_id::text,'poi') ON CONFLICT DO NOTHING;
    ELSE
        DELETE FROM public.place_categories pc
          WHERE pc.place_id::text=apply_poi_membership.entity_id::text AND pc.category_id='poi';
    END IF;
    IF NOT scoped THEN
        -- Outside the pilot, preserve the existing job's full normalized refresh.
        SELECT COALESCE(array_agg(pc.category_id::text),ARRAY[]::text[]) INTO next_categories
          FROM public.place_categories pc WHERE pc.place_id::text=apply_poi_membership.entity_id::text;
    END IF;
    UPDATE public.places p SET categories=next_categories
      WHERE p.id::text=apply_poi_membership.entity_id::text;
    result:=jsonb_build_object('producer','places_poi_category_update_v1','entity_id',entity_id::text,'poi',poi);
    IF scoped THEN PERFORM migration_capture.record_command(result); END IF;
    RETURN result;
END $$;

-- Observe only POI membership, not complete catalogue data. An unmarked writer
-- creates evidence that blocks the stream rather than silently escaping it.
CREATE FUNCTION migration_capture.poi_statement() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
BEGIN
    IF TG_OP='TRUNCATE' THEN RAISE EXCEPTION 'TRUNCATE unsupported while POI capture is installed'; END IF;
    PERFORM pg_advisory_xact_lock(179321,1);
    RETURN NULL;
END $$;
CREATE FUNCTION migration_capture.poi_row() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
DECLARE old_scoped boolean:=false; new_scoped boolean:=false; old_poi boolean:=false; new_poi boolean:=false; p bigint;
BEGIN
    IF TG_OP<>'INSERT' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture.scope s WHERE s.entity_id::text=lower(OLD.id::text)) INTO old_scoped;
        old_poi:=COALESCE('poi'=ANY(OLD.categories),false);
    END IF;
    IF TG_OP<>'DELETE' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture.scope s WHERE s.entity_id::text=lower(NEW.id::text)) INTO new_scoped;
        new_poi:=COALESCE('poi'=ANY(NEW.categories),false);
    END IF;
    IF old_scoped AND old_poi AND (TG_OP='DELETE' OR NOT new_scoped OR OLD.id IS DISTINCT FROM NEW.id) THEN
        p:=migration_capture.ensure_transaction();
        INSERT INTO migration_capture.changes(position,payload) VALUES(p,jsonb_build_object('op','poi_membership','entity_id',OLD.id::text,'poi',false));
    END IF;
    IF new_scoped AND ((TG_OP='INSERT' AND new_poi) OR (TG_OP='UPDATE' AND (old_poi IS DISTINCT FROM new_poi OR (new_poi AND OLD.id IS DISTINCT FROM NEW.id)))) THEN
        p:=migration_capture.ensure_transaction();
        INSERT INTO migration_capture.changes(position,payload) VALUES(p,jsonb_build_object('op','poi_membership','entity_id',NEW.id::text,'poi',new_poi));
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER migration_capture_poi_statement BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.places
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture.poi_statement();
CREATE TRIGGER migration_capture_poi_row AFTER INSERT OR UPDATE OR DELETE ON public.places
FOR EACH ROW EXECUTE FUNCTION migration_capture.poi_row();
