import QtQuick

Text {
    id: root
    renderType: Text.NativeRendering
    verticalAlignment: Text.AlignVCenter

    font {
        family: (typeof Appearance !== "undefined" && Appearance.font && Appearance.font.family && Appearance.font.family.main) ? Appearance.font.family.main : "sans-serif"
        pixelSize: (typeof Appearance !== "undefined" && Appearance.font && Appearance.font.pixelSize && Appearance.font.pixelSize.small) ? Appearance.font.pixelSize.small : 14
    }
    color: (typeof Appearance !== "undefined" && Appearance.colors && Appearance.colors.colOnSurface) ? Appearance.colors.colOnSurface : Qt.rgba(0.90, 0.90, 0.93, 1.0)
}
