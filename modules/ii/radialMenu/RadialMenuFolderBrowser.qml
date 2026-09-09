pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import Quickshell.Io

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenuFolderBrowser — In-Dial Directory Picker Modal Window
// Completely self-contained, appears on top of radial dial without closing it
// ─────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property bool active: false
    property string currentPath: ""
    property string currentName: ""
    property string parentPath: ""
    property var crumbs: []
    property var places: []
    property var folders: []
    property string filterQuery: ""
    property bool loading: false

    property string pendingNavPath: ""

    signal folderSelected(string path, string name)
    signal cancelled()

    readonly property string scriptPath: {
        const u = Qt.resolvedUrl("folder_browser.py").toString()
        return u.replace(/^file:\/\//, "")
    }

    // Universal theme fallbacks
    readonly property color colPrimary: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colPrimary : Qt.rgba(0.66, 0.78, 0.98, 1.0)
    readonly property color colOnPrimary: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOnPrimary : Qt.rgba(0.02, 0.18, 0.44, 1.0)
    readonly property color colOnSurface: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOnSurface : Qt.rgba(0.90, 0.90, 0.93, 1.0)
    readonly property color colSubtext: (typeof Appearance !== "undefined" && Appearance.colors && Appearance.colors.colSubtext) ? Appearance.colors.colSubtext : Qt.rgba(0.70, 0.70, 0.75, 0.8)
    readonly property color colSurfaceContainer: (typeof Appearance !== "undefined" && Appearance.colors && Appearance.colors.colSurfaceContainerHigh) ? Appearance.colors.colSurfaceContainerHigh : Qt.rgba(0.09, 0.09, 0.13, 0.96)
    readonly property color colOutline: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOutline : Qt.rgba(1.0, 1.0, 1.0, 0.14)
    readonly property string fontMain: (typeof Appearance !== "undefined" && Appearance.font && Appearance.font.family) ? Appearance.font.family.main : "sans-serif"
    readonly property string fontIcon: (typeof Appearance !== "undefined" && Appearance.font && Appearance.font.family) ? Appearance.font.family.iconMaterial : "Material Symbols Rounded"

    visible: opacity > 0.001
    opacity: active ? 1.0 : 0.0
    scale: active ? 1.0 : 0.94

    Behavior on opacity { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
    Behavior on scale { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }

    // Backend directory scanner process
    Process {
        id: listProc
        command: ["python3", root.scriptPath, "list", root.pendingNavPath]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                root.loading = false
                try {
                    const data = JSON.parse(text.trim())
                    if (data && data.path) {
                        root.currentPath = data.path
                        root.currentName = data.name || ""
                        root.parentPath = data.parent || ""
                        root.crumbs = data.crumbs || []
                        root.folders = data.folders || []
                    }
                } catch(e) {
                    console.log("[FolderBrowser] parse error:", e)
                }
            }
        }
    }

    // Backend places scanner process
    Process {
        id: placesProc
        command: ["python3", root.scriptPath, "places"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.places = JSON.parse(text.trim()) || []
                } catch(e) {
                    console.log("[FolderBrowser] places error:", e)
                }
            }
        }
    }

    function open(initialPath) {
        root.pendingNavPath = initialPath || "~"
        root.filterQuery = ""
        root.loading = true
        root.active = true
        placesProc.running = true
        listProc.running = true
    }

    function navigateTo(targetPath) {
        if (!targetPath) return
        root.pendingNavPath = targetPath
        root.filterQuery = ""
        root.loading = true
        listProc.running = true
    }

    function close() {
        root.active = false
        root.cancelled()
    }

    function selectCurrent() {
        if (root.currentPath) {
            root.folderSelected(root.currentPath, root.currentName)
        }
        root.active = false
    }

    // Catch backdrop clicks
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: root.close()
    }

    // Filtered subfolders list
    readonly property var filteredFolders: {
        if (!root.folders) return []
        if (!root.filterQuery || root.filterQuery.trim() === "") return root.folders
        const q = root.filterQuery.toLowerCase().trim()
        return root.folders.filter(f => f.name.toLowerCase().includes(q))
    }

    // Main Modal Card Window
    Rectangle {
        id: card
        width: Math.min(parent.width - 40, 540)
        height: Math.min(parent.height - 40, 580)
        anchors.centerIn: parent
        radius: 22
        color: Qt.rgba(0.09, 0.09, 0.13, 0.98)
        border.color: root.colOutline
        border.width: 1.2

        // Prevent click-through
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: {}
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 20
            spacing: 12

            // ── 1. Header ────────────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                // Back / Parent button
                Rectangle {
                    width: 36
                    height: 36
                    radius: 18
                    color: navBackHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.14) : Qt.rgba(1.0, 1.0, 1.0, 0.06)
                    border.color: Qt.rgba(1.0, 1.0, 1.0, 0.12)
                    border.width: 1

                    MaterialSymbol {
                        anchors.centerIn: parent
                        text: (root.parentPath && root.parentPath !== root.currentPath) ? "arrow_back" : "folder_open"
                        iconSize: 18
                        color: root.colPrimary
                    }

                    HoverHandler { id: navBackHover }
                    TapHandler {
                        onTapped: {
                            if (root.parentPath && root.parentPath !== root.currentPath) {
                                root.navigateTo(root.parentPath)
                            }
                        }
                    }
                }

                // Title & current folder name
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    StyledText {
                        text: "Select Folder"
                        font.pixelSize: 15
                        font.weight: Font.Bold
                        color: root.colOnSurface
                    }

                    StyledText {
                        text: root.currentName ? root.currentName : "Browse directories"
                        font.pixelSize: 11
                        color: root.colSubtext
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }

                // Close Button
                Rectangle {
                    width: 32
                    height: 32
                    radius: 16
                    color: closeHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.14) : Qt.rgba(1.0, 1.0, 1.0, 0.06)

                    MaterialSymbol {
                        anchors.centerIn: parent
                        text: "close"
                        iconSize: 16
                        color: root.colOnSurface
                    }

                    HoverHandler { id: closeHover }
                    TapHandler { onTapped: root.close() }
                }
            }

            // ── 2. Current Path Bar & Up Navigation ───────────────────────────
            Rectangle {
                Layout.fillWidth: true
                height: 38
                radius: 10
                color: Qt.rgba(0.14, 0.14, 0.18, 0.9)
                border.color: Qt.rgba(1.0, 1.0, 1.0, 0.12)
                border.width: 1

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    spacing: 6

                    // Up one level button
                    Rectangle {
                        width: 26
                        height: 26
                        radius: 6
                        color: upHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.18) : "transparent"

                        MaterialSymbol {
                            anchors.centerIn: parent
                            text: "arrow_upward"
                            iconSize: 16
                            color: root.parentPath ? root.colPrimary : Qt.rgba(1.0, 1.0, 1.0, 0.3)
                        }

                        HoverHandler { id: upHover }
                        TapHandler {
                            onTapped: {
                                if (root.parentPath) root.navigateTo(root.parentPath)
                            }
                        }
                    }

                    // Path text display
                    StyledText {
                        Layout.fillWidth: true
                        text: root.currentPath || "~"
                        font.pixelSize: 11
                        font.family: "monospace"
                        color: root.colOnSurface
                        elide: Text.ElideMiddle
                    }

                    // Refresh Button
                    Rectangle {
                        width: 26
                        height: 26
                        radius: 6
                        color: refreshHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.18) : "transparent"

                        MaterialSymbol {
                            anchors.centerIn: parent
                            text: "refresh"
                            iconSize: 16
                            color: root.colSubtext
                        }

                        HoverHandler { id: refreshHover }
                        TapHandler {
                            onTapped: root.navigateTo(root.currentPath)
                        }
                    }
                }
            }

            // ── 3. Places & Drives Quick Navigation ──────────────────────────
            ScrollView {
                Layout.fillWidth: true
                height: 36
                contentHeight: 36
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AlwaysOff
                clip: true

                Row {
                    spacing: 6

                    Repeater {
                        model: root.places

                        delegate: Rectangle {
                            id: placeChip
                            required property var modelData

                            height: 32
                            width: placeRow.implicitWidth + 18
                            radius: 16

                            readonly property bool isSelected: root.currentPath === placeChip.modelData.path

                            color: isSelected
                                ? root.colPrimary
                                : (placeHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.14) : Qt.rgba(1.0, 1.0, 1.0, 0.07))
                            border.color: isSelected
                                ? root.colPrimary
                                : Qt.rgba(1.0, 1.0, 1.0, 0.12)
                            border.width: 1

                            RowLayout {
                                id: placeRow
                                anchors.centerIn: parent
                                spacing: 5

                                MaterialSymbol {
                                    text: placeChip.modelData.icon || "folder"
                                    iconSize: 14
                                    color: placeChip.isSelected ? root.colOnPrimary : root.colPrimary
                                }

                                StyledText {
                                    text: placeChip.modelData.name
                                    font.pixelSize: 11
                                    font.weight: placeChip.isSelected ? Font.Bold : Font.Normal
                                    color: placeChip.isSelected ? root.colOnPrimary : root.colOnSurface
                                }
                            }

                            HoverHandler { id: placeHover }
                            TapHandler {
                                onTapped: root.navigateTo(placeChip.modelData.path)
                            }
                        }
                    }
                }
            }

            // ── 4. Search Filter Input ───────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                height: 34
                radius: 10
                color: Qt.rgba(0.12, 0.12, 0.16, 0.7)
                border.color: Qt.rgba(1.0, 1.0, 1.0, 0.10)
                border.width: 1

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 8
                    spacing: 6

                    MaterialSymbol {
                        text: "search"
                        iconSize: 15
                        color: Qt.rgba(1.0, 1.0, 1.0, 0.4)
                    }

                    TextInput {
                        id: filterBox
                        Layout.fillWidth: true
                        font.pixelSize: 12
                        color: root.colOnSurface
                        text: root.filterQuery
                        onTextChanged: root.filterQuery = text

                        Text {
                            text: "Filter subfolders..."
                            visible: !filterBox.text && !filterBox.activeFocus
                            font.pixelSize: 12
                            color: Qt.rgba(1.0, 1.0, 1.0, 0.3)
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }

                    Rectangle {
                        visible: filterBox.text.length > 0
                        width: 20
                        height: 20
                        radius: 10
                        color: Qt.rgba(1.0, 1.0, 1.0, 0.15)

                        MaterialSymbol {
                            anchors.centerIn: parent
                            text: "close"
                            iconSize: 12
                            color: root.colOnSurface
                        }

                        TapHandler { onTapped: filterBox.text = "" }
                    }
                }
            }

            // ── 5. Folders List ──────────────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: 12
                color: Qt.rgba(0.06, 0.06, 0.08, 0.5)
                border.color: Qt.rgba(1.0, 1.0, 1.0, 0.08)
                border.width: 1
                clip: true

                ListView {
                    id: folderListView
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 2
                    model: root.filteredFolders
                    ScrollBar.vertical: ScrollBar { active: true; width: 6 }

                    delegate: Rectangle {
                        id: folderItem
                        required property var modelData

                        width: folderListView.width - 8
                        height: 38
                        radius: 8
                        color: itemHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.12) : "transparent"

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 10

                            MaterialSymbol {
                                text: "folder"
                                iconSize: 18
                                color: root.colPrimary
                            }

                            StyledText {
                                Layout.fillWidth: true
                                text: folderItem.modelData.name
                                font.pixelSize: 12
                                color: root.colOnSurface
                                elide: Text.ElideRight
                            }

                            MaterialSymbol {
                                text: "chevron_right"
                                iconSize: 16
                                color: Qt.rgba(1.0, 1.0, 1.0, 0.25)
                            }
                        }

                        HoverHandler { id: itemHover }
                        TapHandler {
                            onTapped: root.navigateTo(folderItem.modelData.path)
                        }
                    }

                    // Empty or Loading indicator
                    Item {
                        anchors.fill: parent
                        visible: root.filteredFolders.length === 0

                        ColumnLayout {
                            anchors.centerIn: parent
                            spacing: 8

                            MaterialSymbol {
                                Layout.alignment: Qt.AlignHCenter
                                text: root.loading ? "progress_activity" : "folder_off"
                                iconSize: 32
                                color: Qt.rgba(1.0, 1.0, 1.0, 0.3)
                            }

                            StyledText {
                                Layout.alignment: Qt.AlignHCenter
                                text: root.loading ? "Scanning directory..." : (root.filterQuery ? "No matching folders" : "No subfolders in this directory")
                                font.pixelSize: 12
                                color: Qt.rgba(1.0, 1.0, 1.0, 0.4)
                            }
                        }
                    }
                }
            }

            // ── 6. Bottom Action Bar ─────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                // Cancel Button
                Rectangle {
                    width: 90
                    height: 40
                    radius: 10
                    color: cancelHover.hovered ? Qt.rgba(1.0, 1.0, 1.0, 0.14) : Qt.rgba(1.0, 1.0, 1.0, 0.08)
                    border.color: Qt.rgba(1.0, 1.0, 1.0, 0.16)
                    border.width: 1

                    StyledText {
                        anchors.centerIn: parent
                        text: "Cancel"
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                        color: root.colOnSurface
                    }

                    HoverHandler { id: cancelHover }
                    TapHandler { onTapped: root.close() }
                }

                Item { Layout.fillWidth: true } // Spacer

                // Primary Select Current Folder Button
                Rectangle {
                    height: 40
                    width: selectRow.implicitWidth + 28
                    radius: 10
                    color: selectHover.hovered ? Qt.lighter(root.colPrimary, 1.15) : root.colPrimary

                    RowLayout {
                        id: selectRow
                        anchors.centerIn: parent
                        spacing: 6

                        MaterialSymbol {
                            text: "check"
                            iconSize: 18
                            color: root.colOnPrimary
                        }

                        StyledText {
                            text: "Select " + (root.currentName ? ('"' + root.currentName + '"') : "Folder")
                            font.pixelSize: 12
                            font.weight: Font.Bold
                            color: root.colOnPrimary
                            elide: Text.ElideRight
                        }
                    }

                    HoverHandler { id: selectHover }
                    TapHandler { onTapped: root.selectCurrent() }
                }
            }
        }
    }
}
