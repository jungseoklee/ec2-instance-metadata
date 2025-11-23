# EC2 Instance Metadata CLI

A CLI tool to retrieve AWS EC2 instance metadata with support for one-time query and polling. It outputs structured JSON with flexible timestamps (ISO 8601 or Unix) and uses IMDSv2.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90+-orange.svg)](https://www.rust-lang.org)

## Installation

### Install

```bash
cargo install --git https://github.com/jungseoklee/ec2-instance-metadata.git
```

### Uninstall

```bash
cargo uninstall ec2-instance-metadata
```

## Requirements

- Must run on an IMDSv2 enabled AWS EC2 instance.
- `curl` must be installed.

> **Note:** The `curl` dependency may be removed in a future version.

## Usage

### Get Command
```
Usage: ec2im get [OPTIONS] <PATH>

Arguments:
  <PATH>

Options:
  -t, --timestamp-format <TIMESTAMP_FORMAT>  [default: iso] [possible values: iso, unix]
  -h, --help                                 Print help
```

### Poll Command
```
Usage: ec2im poll [OPTIONS] <PATH>

Arguments:
  <PATH>

Options:
  -i, --interval <INTERVAL>                  [default: 5000]
  -t, --timestamp-format <TIMESTAMP_FORMAT>  [default: iso] [possible values: iso, unix]
  -h, --help
```

## Examples

### Query Instance ID

```bash
ec2im get meta-data/instance-id
```

**Output:**
```json
{"timestamp":"2025-11-25T00:06:11.614","status":"success","value":"i-0b22a22eec53b9321"}
```

### Query with Unix Timestamp

```bash
ec2im get meta-data/ami-id --timestamp-format unix
```

**Output:**
```json
{"timestamp": 1764029142997,"status":"success","value":"ami-0abcdef1234567890"}
```

### Poll Auto Scaling Target Lifecycle State Every 3 Seconds

```bash
ec2im poll meta-data/autoscaling/target-lifecycle-state --interval 3000
```

**Output:**
```json
{"timestamp":"2025-11-25T00:06:34.925","status":"success","value":"InService"}
{"timestamp":"2025-11-25T00:06:37.936","status":"success","value":"InService"}
{"timestamp":"2025-11-25T00:06:40.946","status":"success","value":"InService"}
```

## Output Format

All responses follow this JSON structure:

**Success:**
```json
{
  "timestamp": "<ISO8601 or Unix>",
  "status": "success",
  "value": "<metadata value>"
}
```

**Error:**
```json
{
  "timestamp": "<ISO8601 or Unix>",
  "status": "error",
  "value": null,
  "reason": "<error message>"
}
```

## Metadata Paths

See [Access instance metadata for an EC2 instance](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/instancedata-data-retrieval.html).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
