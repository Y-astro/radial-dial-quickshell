import QtQuick
import qs.modules.common

Text {
    id: root
    property real iconSize: 18
    property real fill: 0
    property real resolvedFill: fill >= 0.5 ? 1.0 : 0.0

    renderType: Text.NativeRendering
    verticalAlignment: Text.AlignVCenter
    horizontalAlignment: Text.AlignHCenter

    font {
        hintingPreference: Font.PreferNoHinting
        family: (typeof Appearance !== "undefined" && Appearance.font && Appearance.font.family && Appearance.font.family.iconMaterial) ? Appearance.font.family.iconMaterial : "Material Symbols Rounded"
        pixelSize: iconSize
        weight: resolvedFill > 0.5 ? Font.DemiBold : Font.Normal
        variableAxes: {
            "FILL": resolvedFill,
            "opsz": iconSize,
        }
    }
}
