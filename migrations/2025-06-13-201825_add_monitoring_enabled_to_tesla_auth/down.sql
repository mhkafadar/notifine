-- Remove monitoring_enabled field from tesla_auth table
ALTER TABLE tesla_auth 
DROP COLUMN monitoring_enabled;