# ESP32-C3 interfacing to a PS/2 Keyboard (bare-metal Rust)

Very simplified example of connecting a PS/2 keyboard to ESP32-C3

You need to build it with the release profile (i.e. `cargo run --release`) since otherwise the interrupt latency is too slow.

![Screenshot](docs/screenshot.png "Screenshot")

## Circuit

- 2 x 2.2kΩ resitors
- 2 x 120Ω resitors
```

    Keyboard                                    ESP32-C3

    5V      ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  5V

    GND     ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  GND

                                        ┏━━━━━┓
    DATA    ━━━━━━━━━━━━━━━━━━━━━┳━━━━━━┫ 120 ┣━━━  IO1
                                 ┃      ┗━━━━━┛ 
                                ┏┻┓ 
                                ┃2┃ 
                                ┃.┃
                                ┃2┃
                                ┃k┃
                                ┗┳┛
                                 ┃
                                 ┣━━━━━━━━━━━━━━━━  3V3
                                 ┃
                                ┏┻┓
                                ┃2┃
                                ┃.┃
                                ┃2┃
                                ┃k┃
                                ┗┳┛
                                 ┃      ┏━━━━━┓
    CLK     ━━━━━━━━━━━━━━━━━━━━━┻━━━━━━┫ 120 ┣━━━  IO2
                                        ┗━━━━━┛                                                

```



