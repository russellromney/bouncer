.load /path/to/liblitelease_ext

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS jobs (payload TEXT NOT NULL);
INSERT INTO jobs(payload) VALUES ('run scheduler tick');

SELECT litelease_bootstrap();
SELECT litelease_claim('scheduler', 'worker-a', 30000, 1700000000000);

COMMIT;
