pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io
import qs
import qs.modules.common
import qs.modules.common.widgets

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenu — Transparent overlay window loader with active keyboard focus
// ─────────────────────────────────────────────────────────────────────────────
Scope {
    id: root

    Loader {
        id: menuLoader
        active: GlobalStates.radialMenuOpen

        sourceComponent: PanelWindow {
            id: menuWin

            screen: GlobalStates.radialMenuScreen ?? Quickshell.screens[0]
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            exclusiveZone: 0

            WlrLayershell.namespace: "quickshell:radialMenu"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            Connections {
                target: GlobalStates
                function onRadialMenuCloseRequested() {
                    radialContent.closeAnimated()
                }
            }

            // Click outside the dial closes the menu with outside-to-inside animation
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                onClicked: radialContent.closeAnimated()
            }

            RadialMenuContent {
                id: radialContent
                centerX: GlobalStates.radialMenuX
                centerY: GlobalStates.radialMenuY
                focus: true
            }
        }
    }
}
