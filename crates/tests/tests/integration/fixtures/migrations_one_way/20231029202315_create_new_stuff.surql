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

DEFINE TABLE planet SCHEMAFULL;
DEFINE FIELD createdAt ON planet TYPE datetime;
DEFINE FIELD firstName ON planet TYPE string;
DEFINE FIELD population ON planet TYPE int;
DEFINE FIELD tags ON planet TYPE array<string>;
DEFINE FIELD updatedAt ON planet TYPE datetime;

DEFINE TABLE migration SCHEMAFULL;
DEFINE FIELD id ON migration TYPE string;
DEFINE FIELD name ON migration TYPE string;
DEFINE FIELD timestamp ON migration TYPE int;
DEFINE FIELD checksum_up ON migration TYPE string;

-- Reset the migration history
DELETE migration;
