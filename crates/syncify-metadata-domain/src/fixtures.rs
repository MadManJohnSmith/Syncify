//! Deterministic offline fixtures for metadata enrichment testing.

pub const FIXTURE_MB_EXACT_RECORDING_JSON: &str = r#"{
  "id": "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d",
  "title": "Heroes",
  "length": 367000,
  "artist-credit": [
    {
      "name": "David Bowie",
      "artist": {
        "id": "5441c29d-3602-48f7-b1a9-30704df52227",
        "name": "David Bowie"
      }
    }
  ],
  "releases": [
    {
      "id": "673752e3-2e06-4447-aa72-a080ef8a1768",
      "title": "Heroes",
      "status": "Official",
      "country": "GB",
      "date": "1977-10-14",
      "barcode": "0035629007421",
      "release-group": {
        "id": "c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061",
        "primary-type": "Album"
      },
      "label-info": [
        {
          "catalog-number": "PL 12522",
          "label": {
            "id": "b8045952-4416-419b-b9f1-d68a98bf4998",
            "name": "RCA Victor"
          }
        }
      ]
    }
  ]
}"#;

pub const FIXTURE_MB_ALTERNATIVE_RELEASE_JSON: &str = r#"{
  "id": "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d",
  "title": "Heroes",
  "length": 367000,
  "artist-credit": [
    {
      "name": "David Bowie",
      "artist": {
        "id": "5441c29d-3602-48f7-b1a9-30704df52227",
        "name": "David Bowie"
      }
    }
  ],
  "releases": [
    {
      "id": "11111111-2e06-4447-aa72-a080ef8a1768",
      "title": "Greatest Hits",
      "status": "Official",
      "country": "US",
      "date": "1990-01-01",
      "barcode": "077779400228",
      "release-group": {
        "id": "22222222-d9c0-3ec6-b33a-bcbbd011f061",
        "primary-type": "Compilation"
      },
      "label-info": [
        {
          "catalog-number": "CDP 7 94002 2",
          "label": {
            "id": "33333333-4416-419b-b9f1-d68a98bf4998",
            "name": "EMI"
          }
        }
      ]
    },
    {
      "id": "673752e3-2e06-4447-aa72-a080ef8a1768",
      "title": "Heroes",
      "status": "Official",
      "country": "GB",
      "date": "1977-10-14",
      "barcode": "0035629007421",
      "release-group": {
        "id": "c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061",
        "primary-type": "Album"
      },
      "label-info": [
        {
          "catalog-number": "PL 12522",
          "label": {
            "id": "b8045952-4416-419b-b9f1-d68a98bf4998",
            "name": "RCA Victor"
          }
        }
      ]
    }
  ]
}"#;

pub const FIXTURE_MB_NOT_FOUND_JSON: &str = r#"{
  "created": "2026-08-12T12:00:00.000Z",
  "count": 0,
  "offset": 0,
  "recordings": []
}"#;
