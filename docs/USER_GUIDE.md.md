````markdown
# LightMon User Guide

**Version:** v1.0-beta

---

## Table of Contents

1. [Introduction](#introduction)  
2. [System Requirements](#system-requirements)  
3. [Installation](#installation)  
4. [Running the Application](#running-the-application)  
5. [Overview Screen](#overview-screen)  
6. [Processes Screen](#processes-screen)  
7. [Settings Screen](#settings-screen)  
8. [Exporting Processes](#exporting-processes)  
9. [Troubleshooting](#troubleshooting)

---

## Introduction

LightMon is a lightweight system monitor built in Rust. It allows you to monitor CPU, memory, disk usage, and running processes in a simple, easy-to-use interface.  

Key features include:  

- Overview of CPU, Memory, and Disk usage with visual bars  
- List of running processes with CPU and memory sorting  
- Process search and selection  
- Export process list to CSV  
- Dark and Light theme support  
- Configurable refresh interval  

---

## System Requirements

- Rust ≥ 1.70  
- Cargo package manager  
- Operating system: Windows 11

---

## Installation

1. Clone the repository:

```bash
git clone https://github.com/yekhaungsoe/lightmon.git
cd lightmon
````

2. Build the application:

```bash
cargo build --release
```

---

## Running the Application

1. Run LightMon:

```bash
cargo run --release
```

2. The main window will appear with the Overview screen.

---

## Overview Screen

The Overview screen provides a summary of your system’s resources:

* **CPU**: Displays current CPU usage with a progress bar
* **Memory**: Displays used and total memory
* **Disk**: Displays disk usage
* **Update Frequency**: Configured in Settings

---

## Processes Screen

The Processes screen lists all running processes.

Features:

* **Sorting**: Sort by CPU or Memory usage
* **Filtering**: Search by process name or PID
* **Process Details**: Click a process to view detailed information
* **Export**: Export the process list to a CSV file

---

## Settings Screen

The Settings screen allows you to customize LightMon:

* **Update Frequency**: Change how often the system data refreshes (in seconds)
* **Theme**: Switch between Light and Dark themes

**Example: Changing refresh interval to 5 seconds**

```text
Enter "5" in the input box and press Enter
```

---

## Exporting Processes

You can export the running processes to a CSV file.

1. Go to the Processes screen
2. Click **Export to CSV**
3. The file will be saved as `processes.csv` in the current working directory

---

## Troubleshooting

* **Application does not start**: Ensure Rust and Cargo are installed and updated.
* **CSV export fails**: Make sure the file is not open in another program and that you have write permissions.
* **Settings not saved**: Verify write permissions for `lightmon_config.toml`.

---

**End of User Guide**

```
