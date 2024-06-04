# Gestures configuration
## Location
The configuration is looked for at `$XDG_CONFIG_HOME/gestures.kdl` and then at
`$XDG_CONFIG_HOME/gestures/gestures.kdl`. If `XDG_CONFIG_HOME` is not set, `$HOME/.config` is used
instead.

## Format
The configuration format (since 0.5.0) uses [`kdl`](https://kdl.dev).

### Swipe Gestures
`swipe` requires a direction and fingers field at least
- `direction` can be one of "nw", "n", "ne", "w", "any", "e", "sw", "s", or "se"
- `fingers` is the number of fingers used to trigger the action
- `start`, `update`, and `end` are all optional. They are executed with `sh -c` and are executed when the gesture is started, recieves an update event, and ends.

Exclusive to `swipe` are two additional options:
- `allow-continue-delay` lets you specify a delay during which you can lift your fingers and continue the gesture before the "end" command is executed. Defaults to no delay.
- `include-cancelled` defines if the "end" command should be executed even if you cancel the gesture, e.g. by switching from 3 to 1 fingers without lifting first. This is useful if your end command is invoking mouse inputs. Defaults to "false".

In all of the fields which execute a shell command, `delta_x`, `delta_y`, `delta_angle`, and `scale` are replaced with the delta in the x and y directions, the angle delta and the scale (movement farther apart or closer together) of the gesture. If they are used for an action in which they do not make sense (e.g. using `scale` in the swipe gesture, `0.0` is used as the value.)
```kdl
swipe direction="s" fingers=4 start="<command>" update="<command>" end="<command>"
```
### Pinch Gestures
`pinch` direction can be "in", "out", "clockwise" or "counter-clockwise". `start`, `update`, and `end` behave the same as `swipe`.
```kdl
pinch direction="out" fingers=3 start="<command>" update="<command>" end="<command>"
```
### Hold Gestures
`hold` only has one `action`, rather than start, end and update, because it does not make much sense to update it.
```kdl
hold fingers=3 action="<command>"
```

## Examples
With the help of [`ydotool`](https://github.com/ReimuNotMoe/ydotool), this config would enable "three finger drag", similar to the feature found on macOS:
```kdl
// allow-continue-delay can be adjusted as desired
swipe direction="any" fingers=3 start="ydotool click -- 0x40" update="ydotool mousemove -- $delta_x $delta_y" end="ydotool click -- 0x80" allow-continue-delay=600 include-cancelled=true
```