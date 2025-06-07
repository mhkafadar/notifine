-- Add monitoring_enabled field to tesla_auth table
ALTER TABLE tesla_auth 
ADD COLUMN monitoring_enabled BOOLEAN NOT NULL DEFAULT TRUE;