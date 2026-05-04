.load /path/to/liblitelease_ext

SELECT litelease_bootstrap();

SELECT litelease_claim('scheduler', 'worker-a', 30000, 1700000000000);
SELECT litelease_owner('scheduler', 1700000000000);
SELECT litelease_token('scheduler');
