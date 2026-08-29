pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets
import qs.modules.common.functions
import qs.modules.ii.radialMenu

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenuContent — Organic Ripple Floating Wedges Radial Menu
// ─────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property real centerX
    required property real centerY

    focus: true

    // State
    property string currentTier: "main"
    property string activeContext: RadialMenuActions.resolveContext(GlobalStates.radialMenuContextWindow)
    property var currentSlices: RadialMenuActions.getSlicesFor(currentTier, activeContext, GlobalStates.radialMenuContextWindow)

    // Geometry (Pixel-aligned integers with floating gaps)
    readonly property int innerRadius: 44
    readonly property int sliceInnerRadius: 54
    readonly property int outerRadius: currentTier === "scratchpad" ? 168 : 152
    readonly property int iconRadius: Math.round((sliceInnerRadius + outerRadius) / 2)
    readonly property int totalRadius: outerRadius + 64

    readonly property int sliceCount: currentSlices ? currentSlices.length : 0
    readonly property real sliceAngle: sliceCount > 0 ? (360.0 / sliceCount) : 360.0

    // Hover state: -1 = none/center, 0..N = slice index
    property int hoveredIndex: -1
    property bool centerHovered: false
    property real hoverFactor: 0.0

    onHoveredIndexChanged: {
        if (hoveredIndex >= 0) {
            hoverSpringAnim.stop()
            hoverFactor = 0.0
            hoverSpringAnim.restart()
        } else {
            hoverFadeAnim.restart()
        }
    }

    NumberAnimation {
        id: hoverSpringAnim
        target: root
        property: "hoverFactor"
        from: 0.0
        to: 1.0
        duration: 160
        easing.type: Easing.OutBack
        easing.overshoot: 1.25
    }

    NumberAnimation {
        id: hoverFadeAnim
        target: root
        property: "hoverFactor"
        to: 0.0
        duration: 120
        easing.type: Easing.OutQuad
    }

    width: totalRadius * 2
    height: totalRadius * 2

    readonly property int cx: Math.round(width / 2)
    readonly property int cy: Math.round(height / 2)

    // Pixel-aligned screen bounds clamping
    x: Math.round(Math.min(Math.max(centerX - cx, 16), (parent?.width ?? 1920) - width - 16))
    y: Math.round(Math.min(Math.max(centerY - cy, 16), (parent?.height ?? 1080) - height - 16))

    // ── Overall Scale and Fade In Transition ──────────────────────────────────
    scale: 0.85
    opacity: 0.0
    Behavior on scale {
        NumberAnimation {
            duration: 200
            easing.type: Easing.OutBack
            easing.overshoot: 1.15
        }
    }
    Behavior on opacity {
        NumberAnimation { duration: 150 }
    }

    // ── Blossom Entrance Animation System ─────────────────────────────────────
    property real hubScale: 0.0
    property real revealProgress: 0.0

    SequentialAnimation {
        id: blossomAnimation
        running: false

        // Step 1: Center Close/Back hub pops up first
        NumberAnimation {
            target: root
            property: "hubScale"
            from: 0.0
            to: 1.0
            duration: 150
            easing.type: Easing.OutBack
            easing.overshoot: 1.3
        }

        // Step 2: Outer floating slices blossom out one by one clockwise
        NumberAnimation {
            target: root
            property: "revealProgress"
            from: 0.0
            to: Math.max(1, root.sliceCount)
            duration: Math.max(220, root.sliceCount * 45)
            easing.type: Easing.OutCubic
        }
    }

    function restartBlossom() {
        hubScale = 0.0
        revealProgress = 0.0
        hoverFactor = 0.0
        blossomAnimation.restart()
    }

    // Trigger animation when opened or tier changed
    onVisibleChanged: {
        if (visible) {
            scale = 1.0
            opacity = 1.0
            currentTier = "main"
            hoveredIndex = -1
            centerHovered = false
            activeContext = RadialMenuActions.resolveContext(GlobalStates.radialMenuContextWindow)
            restartBlossom()
        } else {
            scale = 0.85
            opacity = 0.0
        }
    }

    onCurrentTierChanged: {
        hoveredIndex = -1
        centerHovered = false
        restartBlossom()
    }

    Component.onCompleted: {
        scale = 1.0
        opacity = 1.0
        forceActiveFocus()
        restartBlossom()
    }

    // Key handling (Escape to go back or close)
    function handleEscape(): bool {
        if (currentTier !== "main") {
            currentTier = "main"
            hoveredIndex = -1
            centerHovered = false
            return true
        }
        GlobalStates.radialMenuOpen = false
        return true
    }

    Keys.onPressed: (event) => {
        if (event.key === Qt.Key_Escape) {
            handleEscape()
            event.accepted = true
        }
    }

    // ── Canvas: GPU-Accelerated Floating Rounded Wedges with Ripple Displacement
    Canvas {
        id: pieCanvas
        anchors.fill: parent
        renderTarget: Canvas.FramebufferObject

        property int _hov: root.hoveredIndex
        property real _hovFactor: root.hoverFactor
        property string _tier: root.currentTier
        property real _rev: root.revealProgress
        property color _primary: Appearance.colors.colPrimary

        on_HovChanged: requestPaint()
        on_HovFactorChanged: requestPaint()
        on_TierChanged: requestPaint()
        on_RevChanged: requestPaint()
        on_PrimaryChanged: requestPaint()

        readonly property real gapDeg: Math.max(3.2, 22.0 / Math.max(1, root.sliceCount))

        // Ripple displacement calculator
        function getSliceDisplacement(i, n, h, factor) {
            if (h < 0 || factor <= 0.0 || n <= 0) {
                return { startShift: 0, endShift: 0, rShift: 0 }
            }

            let d = i - h
            while (d > n / 2) d -= n
            while (d < -n / 2) d += n

            const expandDeg = 3.2 * factor

            if (d === 0) {
                return {
                    startShift: -expandDeg,
                    endShift: expandDeg,
                    rShift: 7.0 * factor
                }
            }

            const absD = Math.abs(d)
            const push = (expandDeg * 1.3 / absD) * (d > 0 ? 1.0 : -1.0)
            const rPush = (2.5 / absD) * factor

            return {
                startShift: push,
                endShift: push,
                rShift: rPush
            }
        }

        function drawFloatingWedge(ctx, cx, cy, r0, r1, th0, th1, cr) {
            const dth0 = cr / r0
            const dth1 = cr / r1

            if ((th1 - th0) <= 2.2 * dth0 || (r1 - r0) <= 2.2 * cr) {
                ctx.beginPath()
                ctx.arc(cx, cy, r1, th0, th1, false)
                ctx.arc(cx, cy, r0, th1, th0, true)
                ctx.closePath()
                return
            }

            ctx.beginPath()

            // 1. Inner concentric arc
            ctx.arc(cx, cy, r0, th0 + dth0, th1 - dth0, false)

            // 2. Corner 1: Inner arc -> Radial End edge
            ctx.quadraticCurveTo(
                cx + r0 * Math.cos(th1), cy + r0 * Math.sin(th1),
                cx + (r0 + cr) * Math.cos(th1), cy + (r0 + cr) * Math.sin(th1)
            )

            // 3. Radial End edge
            ctx.lineTo(
                cx + (r1 - cr) * Math.cos(th1),
                cy + (r1 - cr) * Math.sin(th1)
            )

            // 4. Corner 2: Radial End edge -> Outer arc
            ctx.quadraticCurveTo(
                cx + r1 * Math.cos(th1), cy + r1 * Math.sin(th1),
                cx + r1 * Math.cos(th1 - dth1), cy + r1 * Math.sin(th1 - dth1)
            )

            // 5. Outer concentric arc
            ctx.arc(cx, cy, r1, th1 - dth1, th0 + dth1, true)

            // 6. Corner 3: Outer arc -> Radial Start edge
            ctx.quadraticCurveTo(
                cx + r1 * Math.cos(th0), cy + r1 * Math.sin(th0),
                cx + (r1 - cr) * Math.cos(th0), cy + (r1 - cr) * Math.sin(th0)
            )

            // 7. Radial Start edge
            ctx.lineTo(
                cx + (r0 + cr) * Math.cos(th0),
                cy + (r0 + cr) * Math.sin(th0)
            )

            // 8. Corner 4: Radial Start edge -> Inner arc
            ctx.quadraticCurveTo(
                cx + r0 * Math.cos(th0), cy + r0 * Math.sin(th0),
                cx + r0 * Math.cos(th0 + dth0), cy + r0 * Math.sin(th0 + dth0)
            )

            ctx.closePath()
        }

        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)

            const cx = root.cx
            const cy = root.cy
            const n = root.sliceCount
            if (n === 0) return

            ctx.lineJoin = "round"
            ctx.lineCap = "round"

            const cr = 6 // Smooth corner radius for rounded wedges

            // ── Individual Floating Rounded Wedges with Ripple Displacement ──
            for (let i = 0; i < n; i++) {
                const p = Math.min(Math.max(root.revealProgress - i, 0.0), 1.0)
                if (p <= 0.0) continue

                const ease = (p >= 1.0) ? 1.0 : (1.0 - Math.pow(1.0 - p, 3))
                const disp = getSliceDisplacement(i, n, root.hoveredIndex, root.hoverFactor)

                const baseOuter = root.sliceInnerRadius + (root.outerRadius - root.sliceInnerRadius) * ease
                const currentOuter = baseOuter + disp.rShift

                const baseStartDeg = i * root.sliceAngle - 90 + gapDeg / 2
                const baseEndDeg = (i + 1) * root.sliceAngle - 90 - gapDeg / 2

                const startDeg = baseStartDeg + disp.startShift
                const endDeg = baseEndDeg + disp.endShift

                const startRad = startDeg * Math.PI / 180
                const endRad = endDeg * Math.PI / 180

                const isHov = (i === root.hoveredIndex)

                // Build smooth rounded floating wedge path
                drawFloatingWedge(ctx, cx, cy, root.sliceInnerRadius, currentOuter, startRad, endRad, cr)

                if (isHov) {
                    // Soft radial gradient highlight pointing toward center
                    const grad = ctx.createRadialGradient(
                        cx, cy, root.sliceInnerRadius - 10,
                        cx, cy, currentOuter + 10
                    )
                    grad.addColorStop(0.0, Qt.lighter(Appearance.colors.colPrimary, 1.35))
                    grad.addColorStop(0.35, Appearance.colors.colPrimary)
                    grad.addColorStop(1.0, Qt.darker(Appearance.colors.colPrimary, 1.15))

                    ctx.fillStyle = grad
                    ctx.fill()
                    ctx.strokeStyle = Qt.lighter(Appearance.colors.colPrimary, 1.25)
                    ctx.lineWidth = 1.6
                    ctx.stroke()
                } else {
                    // Frosted dark background with subtle radial gradient pointing toward center
                    const grad = ctx.createRadialGradient(
                        cx, cy, root.sliceInnerRadius,
                        cx, cy, currentOuter
                    )
                    grad.addColorStop(0.0, Qt.rgba(0.08, 0.08, 0.12, 0.44))
                    grad.addColorStop(1.0, Qt.rgba(0.04, 0.04, 0.06, 0.34))

                    ctx.fillStyle = grad
                    ctx.fill()
                    ctx.fillStyle = Qt.rgba(1.0, 1.0, 1.0, 0.05)
                    ctx.fill()
                    ctx.strokeStyle = Qt.rgba(1.0, 1.0, 1.0, 0.18)
                    ctx.lineWidth = 1.0
                    ctx.stroke()
                }
            }
        }
    }

    // ── High-Efficiency Mouse Tracking Area ───────────────────────────────────
    MouseArea {
        id: dialMouseArea
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton

        onPositionChanged: (mouse) => {
            const dx = mouse.x - root.cx
            const dy = mouse.y - root.cy
            const distSq = dx * dx + dy * dy

            const innerSq = root.innerRadius * root.innerRadius
            const outerSq = (root.outerRadius + 14) * (root.outerRadius + 14)

            if (distSq < innerSq) {
                if (root.hoveredIndex !== -1) root.hoveredIndex = -1
                if (!root.centerHovered) root.centerHovered = true
                return
            }

            if (root.centerHovered) root.centerHovered = false

            if (distSq > outerSq || distSq < root.sliceInnerRadius * root.sliceInnerRadius * 0.8) {
                if (distSq > outerSq && root.hoveredIndex !== -1) root.hoveredIndex = -1
                return
            }

            let angle = Math.atan2(dy, dx) * 180.0 / Math.PI + 90.0
            if (angle < 0) angle += 360.0

            const newIdx = Math.floor(angle / root.sliceAngle) % root.sliceCount
            if (newIdx !== root.hoveredIndex) {
                root.hoveredIndex = newIdx
            }
        }

        onExited: {
            if (root.hoveredIndex !== -1) root.hoveredIndex = -1
            if (root.centerHovered) root.centerHovered = false
        }

        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                root.handleEscape()
                return
            }

            const dx = mouse.x - root.cx
            const dy = mouse.y - root.cy
            const distSq = dx * dx + dy * dy

            // Click in center hub -> Go back or close
            if (distSq <= root.sliceInnerRadius * root.sliceInnerRadius) {
                root.handleEscape()
                return
            }

            // Click on active slice
            if (root.hoveredIndex >= 0 && root.hoveredIndex < root.sliceCount) {
                const slice = root.currentSlices[root.hoveredIndex]
                if (!slice) return
                if (slice.hasSubTier) {
                    root.currentTier = slice.subTierType
                } else if (typeof slice.action === "function") {
                    slice.action()
                    GlobalStates.radialMenuOpen = false
                }
            }
        }
    }

    // ── Slice Icons & Badges Overlay (Synchronized Ripple Physics) ────────────
    Repeater {
        model: root.sliceCount

        delegate: Item {
            id: sliceOverlay
            required property int index

            readonly property var disp: pieCanvas.getSliceDisplacement(sliceOverlay.index, root.sliceCount, root.hoveredIndex, root.hoverFactor)

            readonly property real baseStartDeg: index * root.sliceAngle - 90 + pieCanvas.gapDeg / 2
            readonly property real baseEndDeg: (index + 1) * root.sliceAngle - 90 - pieCanvas.gapDeg / 2

            readonly property real curStartDeg: baseStartDeg + disp.startShift
            readonly property real curEndDeg: baseEndDeg + disp.endShift
            readonly property real midRad: ((curStartDeg + curEndDeg) / 2) * Math.PI / 180

            readonly property real currentIconRadius: root.iconRadius + disp.rShift / 2
            readonly property real rawIconX: root.cx + currentIconRadius * Math.cos(midRad)
            readonly property real rawIconY: root.cy + currentIconRadius * Math.sin(midRad)

            readonly property real rawLabelX: root.cx + (root.outerRadius + disp.rShift + 22) * Math.cos(midRad)
            readonly property real rawLabelY: root.cy + (root.outerRadius + disp.rShift + 22) * Math.sin(midRad)

            readonly property bool isHovered: sliceOverlay.index === root.hoveredIndex
            readonly property var sliceData: root.currentSlices && root.currentSlices[sliceOverlay.index] ? root.currentSlices[sliceOverlay.index] : null

            // Staggered reveal progress
            readonly property real sliceProgress: Math.min(Math.max(root.revealProgress - sliceOverlay.index, 0.0), 1.0)
            readonly property real sliceScale: (sliceProgress >= 1.0) ? 1.0 : (1.0 - Math.pow(1.0 - sliceProgress, 3))

            visible: sliceProgress > 0.0
            opacity: sliceProgress
            scale: sliceScale

            width: 44
            height: 44
            x: Math.round(rawIconX - width / 2)
            y: Math.round(rawIconY - height / 2)

            Behavior on x { NumberAnimation { duration: 110; easing.type: Easing.OutQuad } }
            Behavior on y { NumberAnimation { duration: 110; easing.type: Easing.OutQuad } }

            // ── Material Symbol Icon (Subtle Scale Transform & Spring Transition) ──
            MaterialSymbol {
                anchors.centerIn: parent
                text: sliceOverlay.sliceData ? sliceOverlay.sliceData.icon : ""
                iconSize: 26
                fill: sliceOverlay.isHovered ? 1 : 0
                color: sliceOverlay.isHovered
                    ? Appearance.colors.colOnPrimary
                    : Qt.rgba(1.0, 1.0, 1.0, 0.95)

                scale: sliceOverlay.isHovered ? 1.18 : 1.0
                Behavior on scale {
                    NumberAnimation {
                        duration: 130
                        easing.type: Easing.OutBack
                        easing.overshoot: 1.35
                    }
                }
                Behavior on color { ColorAnimation { duration: 90 } }
                Behavior on fill { NumberAnimation { duration: 90 } }
            }

            // ── Hover Tooltip / Label Pill ──
            Rectangle {
                visible: sliceOverlay.isHovered && !!sliceOverlay.sliceData?.label && sliceOverlay.sliceProgress >= 0.95
                opacity: visible ? 1.0 : 0.0
                radius: Appearance.rounding.small
                color: Qt.rgba(0.08, 0.08, 0.10, 0.90)
                border.color: Qt.rgba(1.0, 1.0, 1.0, 0.20)
                border.width: 1

                width: Math.round(labelText.implicitWidth + 16)
                height: Math.round(labelText.implicitHeight + 8)

                // Position badge aligned to integer pixel boundaries
                x: Math.round((sliceOverlay.rawLabelX - sliceOverlay.x) - width / 2)
                y: Math.round((sliceOverlay.rawLabelY - sliceOverlay.y) - height / 2)

                StyledText {
                    id: labelText
                    anchors.centerIn: parent
                    text: sliceOverlay.sliceData?.label ?? ""
                    font.pixelSize: Appearance.font.pixelSize.small
                    color: Qt.rgba(0.96, 0.96, 0.98, 1.0)
                }
            }
        }
    }

    // ── Floating Center Hub Button (Pixel-Aligned) ────────────────────────────
    Rectangle {
        id: centerBtn
        anchors.centerIn: parent
        width: root.innerRadius * 2
        height: root.innerRadius * 2
        radius: width / 2
        color: root.centerHovered ? Qt.rgba(1.0, 1.0, 1.0, 0.20) : Qt.rgba(0.06, 0.06, 0.08, 0.42)
        border.color: Qt.rgba(1.0, 1.0, 1.0, 0.22)
        border.width: 1.5

        scale: (root.hubScale >= 1.0) ? (root.centerHovered ? 1.06 : 1.0) : root.hubScale
        opacity: root.hubScale

        Behavior on color { ColorAnimation { duration: 60 } }

        MaterialSymbol {
            anchors.centerIn: parent
            text: root.currentTier === "main" ? "close" : "arrow_back"
            iconSize: root.centerHovered ? 24 : 22
            color: Qt.rgba(1.0, 1.0, 1.0, 0.95)
            Behavior on iconSize { NumberAnimation { duration: 60 } }
        }
    }
}
