# Mia HID Pordo

A simple GTK+ tool for inspecting and communicating with USB HID devices.
Intended for electrical engineers who need to test HID reports, not for
software developers.

## Features

- **Device list** – shows all connected HID devices with VID:PID, manufacturer
  and product name.
- **Connect / Disconnect** – opens the selected device and reads its HID
  Report Descriptor.
- **Report tabs** – for every Report ID found in the descriptor, a separate
  tab is created. If the device has no Report ID, a single “Default” tab is
  shown.
- **Input monitoring** – automatically receives and displays interrupt IN
  reports. Each tab shows its own report ID.
- **Manual Read** – sends a “Get Input Report” request and displays the
  response (with timeout).
- **Send Output** – sends an Output report to the device.
- **Set Feature** – sends a Feature report to the device.
- **Get Feature** – requests a Feature report from the device.
- **HEX / ASCII mode** – switch between hex byte display and printable ASCII
  text. Affects both input and output fields.
- **Pause / Resume** – temporarily stops the input reader without
  disconnecting.
- **Show Report Descriptor** – prints the raw HID Report Descriptor in a
  readable, human‑friendly format inside the log.
- **Clear logs** – clears all log tabs at once.
- **Font size** – adjustable log font size (6–48 pt).
- **Auto‑connect** – remembers the last used device and reconnects
  automatically on startup.
- **Settings persistence** – saves window size/position, HEX mode, font size,
  auto‑connect and last device in `~/.config/Mia HID Pordo.cfg`.

## Typical Use

1. Start the program.
2. Select your HID device from the drop‑down list.
3. Click **Connect**.
4. Use the tabs to:
   - watch incoming input reports,
   - send output reports,
   - set or get feature reports.
5. Toggle **HEX** to enter/read data as hex bytes (e.g. `01 02 FF`) or as
   plain text.
6. Click **Show Descriptor** to inspect the HID Report Descriptor.
7. Use **Pause** to freeze the input log while you analyse data.
8. Click **Disconnect** when done.

## Notes

- The device must be accessible by your user account (correct udev rules or
  run as root).
- Maximum log size per tab is 500 lines (oldest lines are removed).
- Feature/Output‑only devices are supported; no input reader is started for
  them.
