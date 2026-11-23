ALTER TABLE IF EXISTS builder.validation_requests DROP CONSTRAINT IF EXISTS validation_requests_trigger_check;
ALTER TABLE IF EXISTS builder.validation_requests ADD CONSTRAINT validation_requests_trigger_check CHECK (trigger IN ('publish', 'changes', 'retry'));
