# dns-updater
A dynamic dns updater for DigitalOcean that sets the specified records to the current WAN ip.

## Usage

```
dns-updater.exe [OPTIONS] --api-key <DO_API_KEY> --domain <DOMAINS>

Options:
  -a, --api-key <DO_API_KEY>
          API key for DigitalOcean
  -m, --update-interval <UPDATE_INTERVAL>
          How often (in minutes) to check WAN IP and update records. If unset the records will only be updated once and then the program will exit
  -A, --apply
          If this flag is **NOT** set the program will only validate that the specified domain records are of type A/AAAA depending on WAN ip type. It will also preview the changes that would be made
  -d, --domain <DOMAINS>
          List of fully qualified domain names to update the values for
  -S, --skip-warning
          If this flag is set the 10 second warning on startup will not be shown before applying record changes
  -h, --help
          Print help
```

### Examples

#### Preview record changes that would be made
```dns-updater --api-key key_with_write_access -d @.example.com -d subdomain.example.com```

#### Update records after verifying it looks good
```dns-updater --api-key key_with_write_access -d @.example.com -d subdomain.example.com -A```

#### Update records and also skip 10 second warning on startup
```dns-updater --api-key key_with_write_access -d @.example.com -AS```

#### Update records now and also skip 10 second warning on startup, and then keep updating records every 30 minutes
```dns-updater --api-key key_with_write_access -d @.example.com -ASm 30```
