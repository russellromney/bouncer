.load /path/to/libbouncer_ext

SELECT bouncer_bootstrap();

SELECT bouncer_claim('scheduler', 'worker-a', 30000, 1700000000000);
SELECT bouncer_owner('scheduler', 1700000000000);
SELECT bouncer_token('scheduler');
