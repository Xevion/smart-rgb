# smart-rgb

A personal script solution to ensure my computer's RGB lights go off when I need them to be.

- Off
  - After idling during the day for more than 3 hours.
  - After idling at night (past 11PM) for more than 25 minutes.
  - Immediately once put into 'sleep' mode.
- On
  - Upon unlock.

This script is intended to be cross-platform for **Windows 10** and **Ubuntu 22.04**.

It uses OpenRGB's server to enable/disable LEDs locally.