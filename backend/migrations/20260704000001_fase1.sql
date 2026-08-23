-- Fase 1: SSL/TLS expiry, confirmations, body validation
ALTER TABLE monitors ADD COLUMN confirmations_required INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitors ADD COLUMN failed_checks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitors ADD COLUMN tls_expiry_days INTEGER;