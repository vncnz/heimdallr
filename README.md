# heimdallr

## About this project

The aim of this project is to show several kinds of information:
- simulate rounded corners of the display
- show which resources are in a worrying state
- show current time
- show estimated time to full battery charge/discharge

After several experiments, I came up with this solution. Using a custom SVG shape, I rounded the screen corners. In the bottom-left corner, the frame reserves space for several icons, visible only when needed, indicating which resources are in a warning state (e.g. RAM almost full, low WLAN signal, and so on).

On the right side, there is a “linear clock” with an arrow indicating the current time. On the same clock, an icon shows the battery’s estimated time to full charge (a green bolt) or to full discharge (a red skull).

All this information takes virtually no useful space on the screen.

Resource data is retrieved via a Linux socket from another of my projects, called Ratatoskr, which is also available on GitHub.

Initially, I implemented this system using the Ignis framework (Python + GTK), but it was consuming about 176 MB of RAM. So I rewrote the UI in Rust, communicating directly with Wayland and avoiding the GTK toolkit. With this approach, memory usage dropped to approximately 34 MB on my laptop.
The impact on average load is around 0.01, so really small. I measured the impact on average load as the ratio between the time spent with the Heimdallr process in "Running" or "disk-sleep" and the total measurement time.

## Screenshots

Light blue border; battery charging; high RAM, medium load, and light disk usage alarms:
![With border, several icons, charging](./screenshots/with_border_and_icons.png)


Light blue border; battery discharging; light disk usage alarm:
![With border, disk icon, skull](./screenshots/with_border_and_skull.png)

No border; battery charging; light disk usage alarm:
![Without border](./screenshots/without_border.png)

No border; battery charging; medium load and light disk usage alarms, with different wallpaper:
![Another wallpaper](./screenshots/another_wallpaper.png)

## TODOs
- Evaluate to use Dunst to manage notifications - get counter, etc.
- Add a visual indicator for Ratatoskr disconnection
- Configurable frame color
- Dynamic frame color (depending on resource icons?)