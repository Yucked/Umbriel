DEFINE TABLE animal SCHEMAFULL;
DEFINE FIELD attributes ON animal TYPE array<string>;
DEFINE FIELD createdAt ON animal TYPE datetime;
DEFINE FIELD species ON animal TYPE string;
DEFINE FIELD updatedAt ON animal TYPE datetime;
DEFINE FIELD velocity ON animal TYPE int;
DEFINE INDEX species_speed_idx ON animal FIELDS species, velocity UNIQUE;
DEFINE EVENT event1 ON animal WHEN species = 'Homo Erectus' AND velocity > 545 THEN (SELECT * FROM crop);

DEFINE TABLE crop SCHEMAFULL;
DEFINE FIELD color ON crop TYPE string;

DEFINE TABLE eats TYPE RELATION IN animal OUT crop SCHEMAFULL;
DEFINE FIELD place ON eats TYPE string;
DEFINE FIELD createdAt ON eats TYPE datetime;

DEFINE TABLE migration SCHEMAFULL;
DEFINE FIELD id ON migration TYPE string;
DEFINE FIELD name ON migration TYPE string;
DEFINE FIELD timestamp ON migration TYPE int;
DEFINE FIELD checksum_up ON migration TYPE string;
DEFINE FIELD checksum_down ON migration TYPE string;

-- Reset the migration history
DELETE migration;
