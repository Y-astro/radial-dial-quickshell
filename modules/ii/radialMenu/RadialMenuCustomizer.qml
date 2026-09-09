pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import QtQuick.Controls.Material
import Quickshell

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenuCustomizer — Interactive In-Menu Segment Customizer & File Manager
// ─────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property bool active: false
    property string mode: "swap" // "swap" | "file_edit" | "file_add"
    property string targetContext: "default"
    property int targetSlotIndex: -1
    property string currentFunctionId: ""
    property var activeSliceIds: []

    // File target fields
    property int fileTargetIndex: -1
    property string fileTargetLabel: ""
    property string fileTargetPath: ""
    property string fileTargetIcon: "folder"

    // Search and filter
    property string filterCategory: "All"
    property string searchQuery: ""

    // Universal theme fallbacks (compatible with any Quickshell environment)
    readonly property color colPrimary: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colPrimary : Qt.rgba(0.66, 0.78, 0.98, 1.0)
    readonly property color colOnPrimary: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOnPrimary : Qt.rgba(0.02, 0.18, 0.44, 1.0)
    readonly property color colOnSurface: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOnSurface : Qt.rgba(0.90, 0.90, 0.93, 1.0)
    readonly property color colSubtext: (typeof Appearance !== "undefined" && Appearance.colors && Appearance.colors.colSubtext) ? Appearance.colors.colSubtext : Qt.rgba(0.70, 0.70, 0.75, 0.8)
    readonly property color colSurfaceContainer: (typeof Appearance !== "undefined" && Appearance.colors && Appearance.colors.colSurfaceContainerHigh) ? Appearance.colors.colSurfaceContainerHigh : Qt.rgba(0.09, 0.09, 0.13, 0.96)
    readonly property color colOutline: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOutline : Qt.rgba(1.0, 1.0, 1.0, 0.14)
    readonly property string fontMain: (typeof Appearance !== "undefined" && Appearance.font) ? Appearance.font.family.main : "sans-serif"
    readonly property string fontIcon: (typeof Appearance !== "undefined" && Appearance.font) ? Appearance.font.family.iconMaterial : "Material Symbols Rounded"

    anchors.fill: parent
    visible: opacity > 0.001
    opacity: active ? 1.0 : 0.0
    scale: active ? 1.0 : 0.92

    Behavior on opacity { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
    Behavior on scale { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }

    // Close on clicking backdrop
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: root.close()
    }

    // Connect to Directory Picker output & Config Changes
    Connections {
        target: RadialMenuActions
        function onDirectoryPicked(path) {
            if (root.active && (root.mode === "file_edit" || root.mode === "file_add")) {
                root.fileTargetPath = path
                if (root.fileTargetLabel === "") {
                    const clean = path.replace(/[\\/]+$/, "")
                    const parts = clean.split(/[\\/]/)
                    root.fileTargetLabel = parts[parts.length - 1] || "Folder"
                }
            }
        }
        function onConfigChanged() {
            root.activeSliceIds = RadialMenuActions.getActiveSliceIds(root.targetContext)
        }
    }

    function openSwapFunction(context: string, slotIndex: int, currentFnId: string) {
        root.mode = "swap"
        root.targetContext = context || "default"
        root.targetSlotIndex = slotIndex
        root.currentFunctionId = currentFnId || ""
        root.activeSliceIds = RadialMenuActions.getActiveSliceIds(root.targetContext)
        root.filterCategory = "All"
        root.searchQuery = ""
        root.active = true
    }

    function openEditFileTarget(targetIndex: int, label: string, path: string, iconName: string) {
        root.mode = "file_edit"
        root.fileTargetIndex = targetIndex
        root.fileTargetLabel = label || ""
        root.fileTargetPath = path || ""
        root.fileTargetIcon = iconName || "folder"
        root.active = true
    }

    function openAddFileTarget() {
        root.mode = "file_add"
        root.fileTargetIndex = -1
        root.fileTargetLabel = ""
        root.fileTargetPath = ""
        root.fileTargetIcon = "folder"
        root.active = true
    }

    function close() {
        root.active = false
    }

    // Main Modal Card
    Rectangle {
        id: card
        width: 480
        height: 540
        anchors.centerIn: parent
        radius: 22
        color: Qt.rgba(0.09, 0.09, 0.13, 0.96)
        border.color: Qt.rgba(1.0, 1.0, 1.0, 0.14)
        border.width: 1.5
        clip: true

        // Catch clicks inside modal
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: {} // consume click
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 12

            // ── Header ────────────────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                Rectangle {
                    width: 38
                    height: 38
                    radius: 12
                    color: Qt.rgba(root.colPrimary.r, root.colPrimary.g, root.colPrimary.b, 0.18)
                    border.color: root.colPrimary
                    border.width: 1.0

                    MaterialSymbol {
                        anchors.centerIn: parent
                        text: root.mode === "swap" ? "swap_horiz" : (root.mode === "file_add" ? "create_new_folder" : "edit")
                        color: root.colPrimary
                        iconSize: 22
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    StyledText {
                        text: root.mode === "swap" ? "Customize Radial Functions" : (root.mode === "file_add" ? "Add Folder Target" : "Edit Folder Target")
                        font.pixelSize: 16
                        font.weight: Font.Bold
                        color: root.colOnSurface
                    }

                    StyledText {
                        text: root.mode === "swap" ? ("Slot " + (root.targetSlotIndex + 1) + " • Click row to replace segment, or Add/Remove from dial") : "Customize folder path, label, and icon"
                        font.pixelSize: 11
                        color: root.colSubtext ?? Qt.rgba(0.7, 0.7, 0.75, 0.8)
                    }
                }

                // Close Button
                Rectangle {
                    width: 32
                    height: 32
                    radius: 16
                    color: closeHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.12) : "transparent"

                    MaterialSymbol {
                        anchors.centerIn: parent
                        text: "close"
                        iconSize: 18
                        color: root.colOnSurface
                    }

                    HoverHandler { id: closeHover }
                    TapHandler { onTapped: root.close() }
                }
            }

            // ── Mode: SWAP FUNCTION ───────────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.mode === "swap"
                spacing: 10

                // Category Chips
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Repeater {
                        model: ["All", "Apps", "Tools", "Media", "Capture", "Window", "System"]
                        delegate: Rectangle {
                            id: catChip
                            required property string modelData
                            height: 26
                            width: catText.implicitWidth + 16
                            radius: 13
                            color: root.filterCategory === modelData ? root.colPrimary : (catHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.10) : Qt.rgba(1.0, 1.0, 1.0, 0.05))
                            border.color: root.filterCategory === modelData ? root.colPrimary : Qt.rgba(1.0, 1.0, 1.0, 0.12)
                            border.width: 1

                            StyledText {
                                id: catText
                                anchors.centerIn: parent
                                text: catChip.modelData
                                font.pixelSize: 11
                                font.weight: root.filterCategory === catChip.modelData ? Font.Bold : Font.Normal
                                color: root.filterCategory === catChip.modelData ? root.colOnPrimary : root.colOnSurface
                            }

                            HoverHandler { id: catHover }
                            TapHandler { onTapped: root.filterCategory = catChip.modelData }
                        }
                    }
                }

                // Search Box
                Rectangle {
                    Layout.fillWidth: true
                    height: 34
                    radius: 10
                    color: Qt.rgba(0.14, 0.14, 0.18, 0.8)
                    border.color: Qt.rgba(1.0, 1.0, 1.0, 0.12)
                    border.width: 1

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 8

                        MaterialSymbol {
                            text: "search"
                            iconSize: 16
                            color: Qt.rgba(1.0, 1.0, 1.0, 0.4)
                        }

                        TextInput {
                            id: searchInput
                            Layout.fillWidth: true
                            font.pixelSize: 12
                            color: root.colOnSurface
                            text: root.searchQuery
                            onTextChanged: root.searchQuery = text
                            clip: true

                            Text {
                                text: "Search function catalogue..."
                                visible: !searchInput.text && !searchInput.activeFocus
                                font.pixelSize: 12
                                color: Qt.rgba(1.0, 1.0, 1.0, 0.35)
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }

                        MaterialSymbol {
                            text: "close"
                            iconSize: 14
                            color: Qt.rgba(1.0, 1.0, 1.0, 0.5)
                            visible: searchInput.text.length > 0
                            TapHandler { onTapped: { searchInput.text = ""; root.searchQuery = "" } }
                        }
                    }
                }

                // Function List
                ListView {
                    id: fnList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4

                    model: {
                        const all = RadialMenuActions.getFunctionCatalogue()
                        const cat = root.filterCategory
                        const q = root.searchQuery.toLowerCase().trim()
                        return all.filter(item => {
                            if (cat !== "All" && item.category !== cat) return false
                            if (q.length > 0) {
                                const l = (item.label || "").toLowerCase()
                                const d = (item.desc || "").toLowerCase()
                                if (!l.includes(q) && !d.includes(q)) return false
                            }
                            return true
                        })
                    }

                    delegate: Rectangle {
                        id: fnItem
                        required property var modelData
                        readonly property bool isActiveInDial: root.activeSliceIds.includes(modelData.id)

                        width: fnList.width
                        height: 52
                        radius: 12
                        color: fnHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.08) : (isActiveInDial ? Qt.rgba(root.colPrimary.r, root.colPrimary.g, root.colPrimary.b, 0.10) : "transparent")
                        border.color: isActiveInDial ? Qt.rgba(root.colPrimary.r, root.colPrimary.g, root.colPrimary.b, 0.45) : "transparent"
                        border.width: 1

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 10

                            Rectangle {
                                width: 34
                                height: 34
                                radius: 10
                                color: fnItem.isActiveInDial ? Qt.rgba(root.colPrimary.r, root.colPrimary.g, root.colPrimary.b, 0.18) : Qt.rgba(1.0, 1.0, 1.0, 0.06)

                                MaterialSymbol {
                                    anchors.centerIn: parent
                                    text: fnItem.modelData.icon || "extension"
                                    iconSize: 20
                                    color: fnItem.isActiveInDial ? root.colPrimary : root.colOnSurface
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1

                                StyledText {
                                    text: fnItem.modelData.label
                                    font.pixelSize: 13
                                    font.weight: Font.DemiBold
                                    color: root.colOnSurface
                                }

                                StyledText {
                                    text: fnItem.modelData.desc || fnItem.modelData.category
                                    font.pixelSize: 10
                                    color: Qt.rgba(1.0, 1.0, 1.0, 0.5)
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }

                            // ── Active Indicator & Remove Button (when function is active in current dial) ──
                            RowLayout {
                                visible: fnItem.isActiveInDial
                                spacing: 6

                                // Active Badge
                                Rectangle {
                                    height: 24
                                    width: activeRow.implicitWidth + 12
                                    radius: 12
                                    color: Qt.rgba(root.colPrimary.r, root.colPrimary.g, root.colPrimary.b, 0.20)
                                    border.color: root.colPrimary
                                    border.width: 1

                                    RowLayout {
                                        id: activeRow
                                        anchors.centerIn: parent
                                        spacing: 4

                                        MaterialSymbol {
                                            text: "check_circle"
                                            iconSize: 13
                                            color: root.colPrimary
                                        }

                                        StyledText {
                                            text: "Active"
                                            font.pixelSize: 10
                                            font.weight: Font.Bold
                                            color: root.colPrimary
                                        }
                                    }
                                }

                                // Remove Button
                                Rectangle {
                                    id: removeBtn
                                    width: 28
                                    height: 28
                                    radius: 14
                                    color: removeHover.hovered ? Qt.rgba(0.95, 0.25, 0.25, 0.35) : Qt.rgba(0.95, 0.25, 0.25, 0.15)
                                    border.color: removeHover.hovered ? "#ff6b6b" : Qt.rgba(0.95, 0.25, 0.25, 0.45)
                                    border.width: 1

                                    MaterialSymbol {
                                        anchors.centerIn: parent
                                        text: "remove"
                                        iconSize: 16
                                        color: "#ff6b6b"
                                    }

                                    HoverHandler { id: removeHover }
                                    TapHandler {
                                        onTapped: {
                                            RadialMenuActions.removeSliceFromDial(root.targetContext, fnItem.modelData.id)
                                        }
                                    }
                                }
                            }

                            // ── Add Button (when function is NOT active in current dial) ──
                            RowLayout {
                                visible: !fnItem.isActiveInDial

                                Rectangle {
                                    id: addBtn
                                    height: 26
                                    width: addRow.implicitWidth + 14
                                    radius: 13
                                    color: addHover.hovered ? Qt.lighter(root.colPrimary, 1.15) : root.colPrimary

                                    RowLayout {
                                        id: addRow
                                        anchors.centerIn: parent
                                        spacing: 3

                                        MaterialSymbol {
                                            text: "add"
                                            iconSize: 14
                                            color: root.colOnPrimary
                                        }

                                        StyledText {
                                            text: "Add"
                                            font.pixelSize: 11
                                            font.weight: Font.Bold
                                            color: root.colOnPrimary
                                        }
                                    }

                                    HoverHandler { id: addHover }
                                    TapHandler {
                                        onTapped: {
                                            RadialMenuActions.addSliceToDial(root.targetContext, fnItem.modelData.id)
                                        }
                                    }
                                }
                            }
                        }

                        // Clicking anywhere on the row body replaces the right-clicked segment
                        HoverHandler { id: fnHover }
                        TapHandler {
                            onTapped: {
                                RadialMenuActions.swapSlice(root.targetContext, root.targetSlotIndex, fnItem.modelData.id)
                                root.close()
                            }
                        }
                    }
                }
            }

            // ── Mode: EDIT / ADD FILE TARGET ───────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.mode === "file_edit" || root.mode === "file_add"
                spacing: 14

                // Label Input
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    StyledText {
                        text: "Display Name"
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        color: Qt.rgba(1.0, 1.0, 1.0, 0.7)
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 38
                        radius: 10
                        color: Qt.rgba(0.14, 0.14, 0.18, 0.8)
                        border.color: Qt.rgba(1.0, 1.0, 1.0, 0.14)
                        border.width: 1

                        TextInput {
                            id: targetLabelInput
                            anchors.fill: parent
                            anchors.margins: 10
                            font.pixelSize: 13
                            color: root.colOnSurface
                            text: root.fileTargetLabel
                            onTextChanged: root.fileTargetLabel = text

                            Text {
                                text: "e.g. Projects, Notes, Work, Music..."
                                visible: !targetLabelInput.text && !targetLabelInput.activeFocus
                                font.pixelSize: 13
                                color: Qt.rgba(1.0, 1.0, 1.0, 0.3)
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }
                    }
                }

                // Path Input + Browse Button
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    StyledText {
                        text: "Target Folder or File Path"
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        color: Qt.rgba(1.0, 1.0, 1.0, 0.7)
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Rectangle {
                            Layout.fillWidth: true
                            height: 38
                            radius: 10
                            color: Qt.rgba(0.14, 0.14, 0.18, 0.8)
                            border.color: Qt.rgba(1.0, 1.0, 1.0, 0.14)
                            border.width: 1

                            TextInput {
                                id: targetPathInput
                                anchors.fill: parent
                                anchors.margins: 10
                                font.pixelSize: 12
                                color: root.colOnSurface
                                text: root.fileTargetPath
                                onTextChanged: root.fileTargetPath = text
                                clip: true

                                Text {
                                    text: "e.g. ~/Projects or /mnt/data/..."
                                    visible: !targetPathInput.text && !targetPathInput.activeFocus
                                    font.pixelSize: 12
                                    color: Qt.rgba(1.0, 1.0, 1.0, 0.3)
                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                            }
                        }

                        // Browse Button
                        Rectangle {
                            width: 90
                            height: 38
                            radius: 10
                            color: browseHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.15) : Qt.rgba(1.0, 1.0, 1.0, 0.08)
                            border.color: Qt.rgba(1.0, 1.0, 1.0, 0.20)
                            border.width: 1

                            RowLayout {
                                anchors.centerIn: parent
                                spacing: 4

                                MaterialSymbol {
                                    text: "folder_open"
                                    iconSize: 16
                                    color: root.colPrimary
                                }

                                StyledText {
                                    text: "Browse"
                                    font.pixelSize: 11
                                    font.weight: Font.Bold
                                    color: root.colOnSurface
                                }
                            }

                            HoverHandler { id: browseHover }
                            TapHandler {
                                onTapped: RadialMenuActions.openDirectoryPicker()
                            }
                        }
                    }
                }

                // Icon Picker
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    StyledText {
                        text: "Choose Icon"
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        color: Qt.rgba(1.0, 1.0, 1.0, 0.7)
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: 8

                        Repeater {
                            model: [
                                "folder", "folder_special", "school", "download",
                                "description", "photo", "movie", "code",
                                "music_note", "home", "terminal", "work",
                                "favorite", "star", "folder_zip", "cloud"
                            ]
                            delegate: Rectangle {
                                id: iconChip
                                required property string modelData
                                width: 36
                                height: 36
                                radius: 10
                                color: root.fileTargetIcon === modelData ? root.colPrimary : (iconHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.12) : Qt.rgba(1.0, 1.0, 1.0, 0.05))
                                border.color: root.fileTargetIcon === modelData ? root.colPrimary : Qt.rgba(1.0, 1.0, 1.0, 0.12)
                                border.width: 1

                                MaterialSymbol {
                                    anchors.centerIn: parent
                                    text: iconChip.modelData
                                    iconSize: 20
                                    color: root.fileTargetIcon === iconChip.modelData ? root.colOnPrimary : root.colOnSurface
                                }

                                HoverHandler { id: iconHover }
                                TapHandler { onTapped: root.fileTargetIcon = iconChip.modelData }
                            }
                        }
                    }
                }

                Item { Layout.fillHeight: true } // Spacer

                // Action Buttons Row (Save / Delete / Cancel)
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    // Delete Target Button (only in edit mode)
                    Rectangle {
                        visible: root.mode === "file_edit"
                        width: 90
                        height: 36
                        radius: 10
                        color: delHover.hovered ? Qt.rgba(0.9, 0.2, 0.2, 0.4) : Qt.rgba(0.9, 0.2, 0.2, 0.2)
                        border.color: Qt.rgba(0.9, 0.2, 0.2, 0.6)
                        border.width: 1

                        RowLayout {
                            anchors.centerIn: parent
                            spacing: 4

                            MaterialSymbol {
                                text: "delete"
                                iconSize: 16
                                color: "#ff6b6b"
                            }

                            StyledText {
                                text: "Delete"
                                font.pixelSize: 11
                                font.weight: Font.Bold
                                color: "#ff6b6b"
                            }
                        }

                        HoverHandler { id: delHover }
                        TapHandler {
                            onTapped: {
                                RadialMenuActions.removeFileJumpTarget(root.fileTargetIndex)
                                root.close()
                            }
                        }
                    }

                    Item { Layout.fillWidth: true } // Spacer

                    // Cancel Button
                    Rectangle {
                        width: 80
                        height: 36
                        radius: 10
                        color: cancelHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.12) : Qt.rgba(1.0, 1.0, 1.0, 0.05)
                        border.color: Qt.rgba(1.0, 1.0, 1.0, 0.14)
                        border.width: 1

                        StyledText {
                            anchors.centerIn: parent
                            text: "Cancel"
                            font.pixelSize: 12
                            color: root.colOnSurface
                        }

                        HoverHandler { id: cancelHover }
                        TapHandler { onTapped: root.close() }
                    }

                    // Save Button
                    Rectangle {
                        width: 100
                        height: 36
                        radius: 10
                        color: saveHover.hovered ? Qt.lighter(root.colPrimary, 1.15) : root.colPrimary

                        RowLayout {
                            anchors.centerIn: parent
                            spacing: 4

                            MaterialSymbol {
                                text: "check"
                                iconSize: 16
                                color: root.colOnPrimary
                            }

                            StyledText {
                                text: root.mode === "file_add" ? "Add Target" : "Save"
                                font.pixelSize: 12
                                font.weight: Font.Bold
                                color: root.colOnPrimary
                            }
                        }

                        HoverHandler { id: saveHover }
                        TapHandler {
                            onTapped: {
                                const l = root.fileTargetLabel.trim() || "Folder"
                                const p = root.fileTargetPath.trim() || "~"
                                const ic = root.fileTargetIcon || "folder"

                                if (root.mode === "file_add") {
                                    RadialMenuActions.addFileJumpTarget(l, p, ic)
                                } else {
                                    RadialMenuActions.updateFileJumpTarget(root.fileTargetIndex, l, p, ic)
                                }
                                root.close()
                            }
                        }
                    }
                }
            }
        }
    }
}
