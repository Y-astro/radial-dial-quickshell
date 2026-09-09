pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenuContent — Localized Symmetrical Petal Fan Radial Menu
// ─────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property real centerX
    required property real centerY

    // Universal theme fallbacks (compatible with any Quickshell environment)
    readonly property color colPrimary: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colPrimary : Qt.rgba(0.66, 0.78, 0.98, 1.0)
    readonly property color colOnPrimary: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOnPrimary : Qt.rgba(0.02, 0.18, 0.44, 1.0)
    readonly property color colOnSurface: (typeof Appearance !== "undefined" && Appearance.colors) ? Appearance.colors.colOnSurface : Qt.rgba(0.90, 0.90, 0.93, 1.0)
    readonly property color colSubtext: (typeof Appearance !== "undefined" && Appearance.colors && Appearance.colors.colSubtext) ? Appearance.colors.colSubtext : Qt.rgba(0.70, 0.70, 0.75, 0.8)
    readonly property string fontMain: (typeof Appearance !== "undefined" && Appearance.font) ? Appearance.font.family.main : "sans-serif"
    readonly property string fontIcon: (typeof Appearance !== "undefined" && Appearance.font) ? Appearance.font.family.iconMaterial : "Material Symbols Rounded"

    property var contextWindow: (typeof GlobalStates !== "undefined" && GlobalStates) ? GlobalStates.radialMenuContextWindow : null

    focus: true

    // Context & Main Slices
    property string activeContext: RadialMenuActions.resolveContext(root.contextWindow)
    property var currentSlices: RadialMenuActions.getSlicesFor("main", activeContext, root.contextWindow)

    // Localized Concentric Outer Sub-Ring State (Opened on Click)
    property string activeSubTier: ""
    property int parentSliceIndex: -1
    property var subSlices: []
    readonly property int subSliceCount: subSlices ? subSlices.length : 0

    // Geometry (Pixel-aligned concentric rings)
    readonly property int innerRadius: 44       // Center Hub radius
    readonly property int sliceInnerRadius: 54  // Main Ring inner radius
    readonly property int outerRadius: 148      // Main Ring outer radius
    readonly property int iconRadius: Math.round((sliceInnerRadius + outerRadius) / 2) // 101px

    readonly property int subInnerRadius: 168   // Concentric Outer Sub-Ring inner radius (clean floating gap from main ring)
    readonly property int subOuterRadius: 228   // Concentric Outer Sub-Ring outer radius (60px thick)
    readonly property int subIconRadius: Math.round((subInnerRadius + subOuterRadius) / 2) // 198px

    readonly property int totalRadius: subOuterRadius + 44 // 272px bounds

    readonly property int sliceCount: currentSlices ? currentSlices.length : 0
    readonly property real sliceAngle: sliceCount > 0 ? (360.0 / sliceCount) : 360.0

    // Localized Fan Geometry (Centered dynamically over the parent slice)
    readonly property real parentMidAngle: parentSliceIndex >= 0 ? ((parentSliceIndex + 0.5) * sliceAngle - 90.0) : 0.0
    readonly property real subSliceWidth: subSliceCount > 0 ? Math.min(36.0, Math.max(22.0, 110.0 / subSliceCount)) : 28.0
    readonly property real subTotalSpan: subSliceCount * subSliceWidth
    readonly property real subStartAngle: parentMidAngle - subTotalSpan / 2.0

    // Hover state: Main Ring
    property int hoveredIndex: -1
    property bool centerHovered: false
    property real hoverFactor: 0.0

    // Hover state: Localized Concentric Sub-Ring
    property int outerHoveredIndex: -1
    property real outerHoverFactor: 0.0
    property real subRevealProgress: 0.0

    // Dynamic Active Hover Label for Center Hub
    readonly property string activeHoverLabel: {
        if (outerHoveredIndex >= 0 && subSlices && subSlices[outerHoveredIndex]) {
            return subSlices[outerHoveredIndex].label || ""
        }
        if (hoveredIndex >= 0 && currentSlices && currentSlices[hoveredIndex]) {
            return currentSlices[hoveredIndex].label || ""
        }
        return ""
    }

    // Helper: Normalize degrees into [0, 360)
    function normalizeDeg(deg: real): real {
        let d = deg % 360.0
        if (d < 0.0) d += 360.0
        return d
    }

    // Direct Sub-Tier Activation Dispatcher (Triggered on Click)
    function updateSubTier(tierName: string, parentIdx: int) {
        if (activeSubTier === tierName && parentSliceIndex === parentIdx && tierName !== "") {
            // Toggle close if clicking same parent slice
            updateSubTier("", -1)
            return
        }

        if (tierName !== "" && parentIdx >= 0) {
            subCollapseAnim.stop()
            activeSubTier = tierName
            parentSliceIndex = parentIdx
            const items = RadialMenuActions.getSlicesFor(tierName, activeContext, root.contextWindow)
            subSlices = (items && items.length > 0) ? items : []
            outerHoveredIndex = -1
            subRevealAnim.to = Math.max(1, subSlices.length)
            subRevealAnim.restart()
        } else {
            subRevealAnim.stop()
            outerHoveredIndex = -1
            if (subRevealProgress > 0.0) {
                subCollapseAnim.restart()
            } else {
                activeSubTier = ""
                parentSliceIndex = -1
                subSlices = []
            }
        }
    }

    // Sub-Dial Blossom Opening Animation (Inside to Outside)
    NumberAnimation {
        id: subRevealAnim
        target: root
        property: "subRevealProgress"
        from: 0.0
        to: Math.max(1, root.subSliceCount)
        duration: Math.max(180, root.subSliceCount * 40)
        easing.type: Easing.OutCubic
    }

    // Sub-Dial Collapse Closing Animation (Outside to Inside)
    SequentialAnimation {
        id: subCollapseAnim
        running: false

        NumberAnimation {
            target: root
            property: "subRevealProgress"
            to: 0.0
            duration: Math.max(130, root.subSliceCount * 26)
            easing.type: Easing.InCubic
        }

        ScriptAction {
            script: {
                root.activeSubTier = ""
                root.parentSliceIndex = -1
                root.subSlices = []
                root.outerHoveredIndex = -1
            }
        }
    }

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

    onOuterHoveredIndexChanged: {
        if (outerHoveredIndex >= 0) {
            outerHoverSpringAnim.stop()
            outerHoverFactor = 0.0
            outerHoverSpringAnim.restart()
        } else {
            outerHoverFadeAnim.restart()
        }
    }

    NumberAnimation {
        id: outerHoverSpringAnim
        target: root
        property: "outerHoverFactor"
        from: 0.0
        to: 1.0
        duration: 140
        easing.type: Easing.OutBack
        easing.overshoot: 1.2
    }

    NumberAnimation {
        id: outerHoverFadeAnim
        target: root
        property: "outerHoverFactor"
        to: 0.0
        duration: 100
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
        updateSubTier("", -1)
        blossomAnimation.restart()
    }

    // Trigger animation when opened
    onVisibleChanged: {
        if (visible) {
            scale = 1.0
            opacity = 1.0
            hoveredIndex = -1
            centerHovered = false
            activeContext = RadialMenuActions.resolveContext(root.contextWindow)
            if (activeContext === "browser") {
                if (typeof GlobalStates !== "undefined" && typeof GlobalStates.refreshTabs === "function") GlobalStates.refreshTabs()
            }
            restartBlossom()
        } else {
            scale = 0.85
            opacity = 0.0
            updateSubTier("", -1)
        }
    }

    Component.onCompleted: {
        scale = 1.0
        opacity = 1.0
        forceActiveFocus()
        if (activeContext === "browser") {
            if (typeof GlobalStates !== "undefined" && typeof GlobalStates.refreshTabs === "function") GlobalStates.refreshTabs()
        }
        restartBlossom()
    }

    property var pendingAction: null

    // ── Outside-to-Inside Collapse / Exit Animation System ────────────────────
    SequentialAnimation {
        id: collapseAnimation
        running: false

        // Step 1: Concentric rings collapse inward (N down to 0)
        ParallelAnimation {
            NumberAnimation {
                target: root
                property: "subRevealProgress"
                to: 0.0
                duration: 100
                easing.type: Easing.InQuad
            }
            NumberAnimation {
                target: root
                property: "revealProgress"
                to: 0.0
                duration: Math.max(120, root.sliceCount * 24)
                easing.type: Easing.InCubic
            }
            NumberAnimation {
                target: root
                property: "scale"
                to: 0.82
                duration: Math.max(130, root.sliceCount * 24 + 20)
                easing.type: Easing.InQuad
            }
            NumberAnimation {
                target: root
                property: "opacity"
                to: 0.0
                duration: Math.max(130, root.sliceCount * 24 + 20)
                easing.type: Easing.InQuad
            }
        }

        // Step 2: Center Close hub pops down to 0
        NumberAnimation {
            target: root
            property: "hubScale"
            to: 0.0
            duration: 80
            easing.type: Easing.InBack
            easing.overshoot: 1.2
        }

        ScriptAction {
            script: {
                if (typeof GlobalStates !== "undefined") GlobalStates.radialMenuOpen = false
                if (typeof root.pendingAction === "function") {
                    const act = root.pendingAction
                    root.pendingAction = null
                    act()
                }
            }
        }
    }

    function closeAnimated(callback) {
        if (collapseAnimation.running) return
        blossomAnimation.stop()
        pendingAction = (typeof callback === "function") ? callback : null
        collapseAnimation.restart()
    }

    // Key handling (Escape to close, 1..9 physical number keys to trigger slices clockwise from 12:00)
    function handleEscape(): bool {
        if (activeSubTier !== "") {
            updateSubTier("", -1)
            return true
        }
        closeAnimated()
        return true
    }

    function handleNumberKey(num: int): bool {
        const targetIdx = num - 1 // 0-indexed (Key 1 -> Slice 0, Key 2 -> Slice 1, ...)

        // Case A: Localized Sub-Dial is currently open
        if (root.activeSubTier !== "" && root.subSliceCount > 0 && root.parentSliceIndex >= 0) {
            if (targetIdx >= 0 && targetIdx < root.subSliceCount) {
                const subSlice = root.subSlices[targetIdx]
                if (subSlice && typeof subSlice.action === "function") {
                    root.outerHoveredIndex = targetIdx
                    root.closeAnimated(subSlice.action)
                    return true
                }
            }
            return false
        }

        // Case B: Main Wheel is active
        if (targetIdx >= 0 && targetIdx < root.sliceCount) {
            const slice = root.currentSlices[targetIdx]
            if (!slice) return false

            if (slice.hasSubTier) {
                // Open / toggle localized petal fan for this slice
                root.hoveredIndex = targetIdx
                root.updateSubTier(slice.subTierType, targetIdx)
                return true
            } else if (typeof slice.action === "function") {
                // Execute direct slice action
                root.hoveredIndex = targetIdx
                root.closeAnimated(slice.action)
                return true
            }
        }

        return false
    }

    Keys.onPressed: (event) => {
        if (event.key === Qt.Key_Escape) {
            if (customizer.active) {
                customizer.close()
                event.accepted = true
                return
            }
            handleEscape()
            event.accepted = true
            return
        }

        if (customizer.active) {
            // Ignore dial number shortcuts when customizer is active
            return
        }

        let num = -1
        if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
            num = event.key - Qt.Key_1 + 1
        } else if (event.key >= Qt.Key_Numpad1 && event.key <= Qt.Key_Numpad9) {
            num = event.key - Qt.Key_Numpad1 + 1
        }

        if (num >= 1 && num <= 9) {
            if (handleNumberKey(num)) {
                event.accepted = true
                return
            }
        }
    }

    // ── Canvas: GPU-Accelerated Concentric Floating Wedges ────────────────────
    Canvas {
        id: pieCanvas
        anchors.fill: parent
        renderTarget: Canvas.FramebufferObject

        property int _hov: root.hoveredIndex
        property real _hovFactor: root.hoverFactor
        property real _rev: root.revealProgress
        property color _primary: root.colPrimary

        property string _subTier: root.activeSubTier
        property real _subRev: root.subRevealProgress
        property int _outerHov: root.outerHoveredIndex
        property real _outerHovFactor: root.outerHoverFactor
        property int _parentIdx: root.parentSliceIndex

        on_HovChanged: requestPaint()
        on_HovFactorChanged: requestPaint()
        on_RevChanged: requestPaint()
        on_PrimaryChanged: requestPaint()

        on_SubTierChanged: requestPaint()
        on_SubRevChanged: requestPaint()
        on_OuterHovChanged: requestPaint()
        on_OuterHovFactorChanged: requestPaint()
        on_ParentIdxChanged: requestPaint()

        readonly property real gapPx: 8.5

        // Ripple displacement calculator for main ring
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

        // Draw floating wedge with uniform parallel gap (constant Euclidean distance at all radii)
        function drawFloatingWedge(ctx, cx, cy, r0, r1, th0, th1, cr) {
            const hg = gapPx / 2.0
            const dth0 = Math.asin(Math.min(0.92, hg / r0))
            const dth1 = Math.asin(Math.min(0.92, hg / r1))

            const th0_in = th0 + dth0
            const th1_in = th1 - dth0
            const th0_out = th0 + dth1
            const th1_out = th1 - dth1

            if ((th1_in - th0_in) <= 0.02 || (r1 - r0) <= 2.0) {
                ctx.beginPath()
                ctx.arc(cx, cy, r1, th0_out, th1_out, false)
                ctx.arc(cx, cy, r0, th1_in, th0_in, true)
                ctx.closePath()
                return
            }

            const effectiveCr = Math.min(cr, (r1 - r0) / 2.2, Math.max(1.0, (th1_in - th0_in) * r0 / 2.5))
            const cth0 = effectiveCr / r0
            const cth1 = effectiveCr / r1

            // Right straight edge endpoints
            const p_in_r_x = cx + r0 * Math.cos(th1_in)
            const p_in_r_y = cy + r0 * Math.sin(th1_in)
            const p_out_r_x = cx + r1 * Math.cos(th1_out)
            const p_out_r_y = cy + r1 * Math.sin(th1_out)

            const dx_r = p_out_r_x - p_in_r_x
            const dy_r = p_out_r_y - p_in_r_y
            const len_r = Math.hypot(dx_r, dy_r) || 1.0
            const vr_x = dx_r / len_r
            const vr_y = dy_r / len_r

            // Left straight edge endpoints
            const p_in_l_x = cx + r0 * Math.cos(th0_in)
            const p_in_l_y = cy + r0 * Math.sin(th0_in)
            const p_out_l_x = cx + r1 * Math.cos(th0_out)
            const p_out_l_y = cy + r1 * Math.sin(th0_out)

            const dx_l = p_out_l_x - p_in_l_x
            const dy_l = p_out_l_y - p_in_l_y
            const len_l = Math.hypot(dx_l, dy_l) || 1.0
            const vl_x = dx_l / len_l
            const vl_y = dy_l / len_l

            ctx.beginPath()

            // 1. Inner arc
            ctx.arc(cx, cy, r0, th0_in + cth0, th1_in - cth0, false)

            // 2. Corner 1: Inner arc -> Right edge
            ctx.quadraticCurveTo(
                p_in_r_x, p_in_r_y,
                p_in_r_x + effectiveCr * vr_x, p_in_r_y + effectiveCr * vr_y
            )

            // 3. Right edge straight line
            ctx.lineTo(
                p_out_r_x - effectiveCr * vr_x, p_out_r_y - effectiveCr * vr_y
            )

            // 4. Corner 2: Right edge -> Outer arc
            ctx.quadraticCurveTo(
                p_out_r_x, p_out_r_y,
                cx + r1 * Math.cos(th1_out - cth1), cy + r1 * Math.sin(th1_out - cth1)
            )

            // 5. Outer arc
            ctx.arc(cx, cy, r1, th1_out - cth1, th0_out + cth1, true)

            // 6. Corner 3: Outer arc -> Left edge
            ctx.quadraticCurveTo(
                p_out_l_x, p_out_l_y,
                p_out_l_x - effectiveCr * vl_x, p_out_l_y - effectiveCr * vl_y
            )

            // 7. Left edge straight line
            ctx.lineTo(
                p_in_l_x + effectiveCr * vl_x, p_in_l_y + effectiveCr * vl_y
            )

            // 8. Corner 4: Left edge -> Inner arc
            ctx.quadraticCurveTo(
                p_in_l_x, p_in_l_y,
                cx + r0 * Math.cos(th0_in + cth0), cy + r0 * Math.sin(th0_in + cth0)
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

            // ── 1. Main Inner Ring Floating Wedges ────────────────────────────
            for (let i = 0; i < n; i++) {
                const p = Math.min(Math.max(root.revealProgress - i, 0.0), 1.0)
                if (p <= 0.0) continue

                const ease = (p >= 1.0) ? 1.0 : (1.0 - Math.pow(1.0 - p, 3))
                const disp = getSliceDisplacement(i, n, root.hoveredIndex, root.hoverFactor)

                const baseOuter = root.sliceInnerRadius + (root.outerRadius - root.sliceInnerRadius) * ease
                const currentOuter = baseOuter + disp.rShift

                const baseStartDeg = i * root.sliceAngle - 90
                const baseEndDeg = (i + 1) * root.sliceAngle - 90

                const startDeg = baseStartDeg + disp.startShift
                const endDeg = baseEndDeg + disp.endShift

                const startRad = startDeg * Math.PI / 180
                const endRad = endDeg * Math.PI / 180

                const isHov = (i === root.hoveredIndex)
                const isParentOfSub = (i === root.parentSliceIndex && root.activeSubTier !== "")

                // Build uniform parallel gap floating wedge
                drawFloatingWedge(ctx, cx, cy, root.sliceInnerRadius, currentOuter, startRad, endRad, cr)

                if (isHov || isParentOfSub) {
                    // Soft radial gradient highlight pointing toward center
                    const grad = ctx.createRadialGradient(
                        cx, cy, root.sliceInnerRadius - 10,
                        cx, cy, currentOuter + 10
                    )
                    grad.addColorStop(0.0, Qt.lighter(root.colPrimary, 1.35))
                    grad.addColorStop(0.35, root.colPrimary)
                    grad.addColorStop(1.0, Qt.darker(root.colPrimary, 1.15))

                    ctx.fillStyle = grad
                    ctx.fill()
                    ctx.strokeStyle = Qt.lighter(root.colPrimary, 1.25)
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

            // ── 2. Localized Symmetrical Sub-Arc Over Parent Segment ──────────
            const m = root.subSliceCount
            if (root.subRevealProgress > 0.0 && m > 0 && root.parentSliceIndex >= 0) {
                const subCr = 5
                const startAngleBase = root.subStartAngle
                const sliceW = root.subSliceWidth

                for (let j = 0; j < m; j++) {
                    const p = Math.min(Math.max(root.subRevealProgress - j, 0.0), 1.0)
                    if (p <= 0.0) continue

                    const subEase = (p >= 1.0) ? 1.0 : (1.0 - Math.pow(1.0 - p, 3))
                    const isOuterHov = (j === root.outerHoveredIndex)
                    const rLift = isOuterHov ? 5.0 * root.outerHoverFactor : 0.0

                    const currentSubOuter = root.subInnerRadius + (root.subOuterRadius - root.subInnerRadius) * subEase + rLift

                    const startDeg = startAngleBase + j * sliceW
                    const endDeg = startDeg + sliceW

                    const startRad = startDeg * Math.PI / 180
                    const endRad = endDeg * Math.PI / 180

                    drawFloatingWedge(ctx, cx, cy, root.subInnerRadius, currentSubOuter, startRad, endRad, subCr)

                    if (isOuterHov) {
                        const grad = ctx.createRadialGradient(
                            cx, cy, root.subInnerRadius - 5,
                            cx, cy, currentSubOuter + 10
                        )
                        grad.addColorStop(0.0, Qt.lighter(root.colPrimary, 1.30))
                        grad.addColorStop(0.40, root.colPrimary)
                        grad.addColorStop(1.0, Qt.darker(root.colPrimary, 1.10))

                        ctx.fillStyle = grad
                        ctx.fill()
                        ctx.strokeStyle = Qt.lighter(root.colPrimary, 1.30)
                        ctx.lineWidth = 1.8
                        ctx.stroke()
                    } else {
                        const grad = ctx.createRadialGradient(
                            cx, cy, root.subInnerRadius,
                            cx, cy, currentSubOuter
                        )
                        grad.addColorStop(0.0, Qt.rgba(0.12, 0.12, 0.16, 0.60))
                        grad.addColorStop(1.0, Qt.rgba(0.06, 0.06, 0.09, 0.48))

                        ctx.fillStyle = grad
                        ctx.fill()
                        ctx.fillStyle = Qt.rgba(1.0, 1.0, 1.0, 0.06)
                        ctx.fill()
                        ctx.strokeStyle = Qt.rgba(1.0, 1.0, 1.0, 0.24)
                        ctx.lineWidth = 1.0
                        ctx.stroke()
                    }
                }
            }
        }
    }

    // ── High-Efficiency Concentric Mouse Tracking Area ────────────────────────
    MouseArea {
        id: dialMouseArea
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton

        onPositionChanged: (mouse) => {
            const dx = mouse.x - root.cx
            const dy = mouse.y - root.cy
            const dist = Math.sqrt(dx * dx + dy * dy)

            // Zone 1: Center Hub (< 48px)
            if (dist < root.sliceInnerRadius * 0.90) {
                if (root.hoveredIndex !== -1) root.hoveredIndex = -1
                if (root.outerHoveredIndex !== -1) root.outerHoveredIndex = -1
                if (!root.centerHovered) root.centerHovered = true
                return
            }

            if (root.centerHovered) root.centerHovered = false

            // Zone 2: Outside entire dial (> 224px)
            if (dist > root.subOuterRadius + 14) {
                if (root.hoveredIndex !== -1) root.hoveredIndex = -1
                if (root.outerHoveredIndex !== -1) root.outerHoveredIndex = -1
                return
            }

            const rawAngle = Math.atan2(dy, dx) * 180.0 / Math.PI
            const canvasAngle = root.normalizeDeg(rawAngle)
            const clockAngle = root.normalizeDeg(rawAngle + 90.0)

            // Zone 3: Localized Outer Sub-Ring (dist >= 152px)
            if (dist >= root.subInnerRadius - 6) {
                if (root.activeSubTier !== "" && root.subSliceCount > 0 && root.parentSliceIndex >= 0) {
                    const normStart = root.normalizeDeg(root.subStartAngle)
                    const relAngle = root.normalizeDeg(canvasAngle - normStart)

                    if (relAngle <= root.subTotalSpan) {
                        const newOuterIdx = Math.min(root.subSliceCount - 1, Math.floor(relAngle / root.subSliceWidth))
                        if (newOuterIdx !== root.outerHoveredIndex) {
                            root.outerHoveredIndex = newOuterIdx
                        }
                        return
                    }
                }
                root.outerHoveredIndex = -1
                return
            }

            // Zone 4: Main Inner Ring (48px <= dist < 152px)
            if (dist < root.subInnerRadius - 6) {
                root.outerHoveredIndex = -1
                const newIdx = Math.floor(clockAngle / root.sliceAngle) % root.sliceCount
                if (newIdx !== root.hoveredIndex) {
                    root.hoveredIndex = newIdx
                }
            }
        }

        onExited: {
            if (root.hoveredIndex !== -1) root.hoveredIndex = -1
            if (root.outerHoveredIndex !== -1) root.outerHoveredIndex = -1
            if (root.centerHovered) root.centerHovered = false
        }

        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                if (customizer.active) {
                    customizer.close()
                    return
                }

                // Sub-tier outer ring right-click (File Jump targets)
                if (root.activeSubTier === "filejump" && root.outerHoveredIndex >= 0 && root.outerHoveredIndex < root.subSliceCount) {
                    const subItem = root.subSlices[root.outerHoveredIndex]
                    if (subItem) {
                        if (subItem.isAddButton) {
                            customizer.openAddFileTarget()
                        } else {
                            customizer.openEditFileTarget(subItem.targetIndex, subItem.label, subItem.targetPath, subItem.icon)
                        }
                        return
                    }
                }

                // Main dial inner ring right-click (Swap function)
                if (root.hoveredIndex >= 0 && root.hoveredIndex < root.sliceCount) {
                    const mainItem = root.currentSlices[root.hoveredIndex]
                    if (mainItem) {
                        customizer.openSwapFunction(root.activeContext, root.hoveredIndex, mainItem.functionId || "")
                        return
                    }
                }

                // Empty space right click -> close
                root.handleEscape()
                return
            }

            const dx = mouse.x - root.cx
            const dy = mouse.y - root.cy
            const dist = Math.sqrt(dx * dx + dy * dy)

            // Click Center Hub -> Close
            if (dist <= root.sliceInnerRadius * 0.90) {
                root.closeAnimated()
                return
            }

            // Click outside dial bounds -> Close
            if (dist > root.subOuterRadius + 14) {
                root.closeAnimated()
                return
            }

            const rawAngle = Math.atan2(dy, dx) * 180.0 / Math.PI
            const canvasAngle = root.normalizeDeg(rawAngle)
            const clockAngle = root.normalizeDeg(rawAngle + 90.0)

            // Click in Localized Outer Sub-Ring
            if (dist >= root.subInnerRadius - 6 && root.activeSubTier !== "" && root.subSliceCount > 0 && root.parentSliceIndex >= 0) {
                const normStart = root.normalizeDeg(root.subStartAngle)
                const relAngle = root.normalizeDeg(canvasAngle - normStart)

                if (relAngle <= root.subTotalSpan) {
                    const clickIdx = Math.min(root.subSliceCount - 1, Math.floor(relAngle / root.subSliceWidth))
                    const subSlice = root.subSlices[clickIdx]
                    if (subSlice) {
                        if (subSlice.isAddButton) {
                            customizer.openAddFileTarget()
                            return
                        }
                        if (typeof subSlice.action === "function") {
                            root.closeAnimated(subSlice.action)
                        }
                    }
                }
                return
            }

            // Click in Main Inner Ring -> Open Sub-Tier or Execute Action
            if (root.hoveredIndex >= 0 && root.hoveredIndex < root.sliceCount) {
                const slice = root.currentSlices[root.hoveredIndex]
                if (!slice) return
                if (slice.hasSubTier) {
                    // Click opens/toggles the localized outer petal fan!
                    root.updateSubTier(slice.subTierType, root.hoveredIndex)
                } else if (typeof slice.action === "function") {
                    root.closeAnimated(slice.action)
                }
            }
        }
    }

    // ── 1. Main Ring Icons Overlay ───────────────────────────────────────────
    Repeater {
        model: root.sliceCount

        delegate: Item {
            id: sliceOverlay
            required property int index

            readonly property var disp: pieCanvas.getSliceDisplacement(sliceOverlay.index, root.sliceCount, root.hoveredIndex, root.hoverFactor)

            readonly property real baseStartDeg: index * root.sliceAngle - 90
            readonly property real baseEndDeg: (index + 1) * root.sliceAngle - 90

            readonly property real curStartDeg: baseStartDeg + disp.startShift
            readonly property real curEndDeg: baseEndDeg + disp.endShift
            readonly property real midRad: ((curStartDeg + curEndDeg) / 2) * Math.PI / 180

            readonly property real currentIconRadius: root.iconRadius + disp.rShift / 2
            readonly property real rawIconX: root.cx + currentIconRadius * Math.cos(midRad)
            readonly property real rawIconY: root.cy + currentIconRadius * Math.sin(midRad)

            readonly property bool isHovered: sliceOverlay.index === root.hoveredIndex
            readonly property bool isParentOfSub: (sliceOverlay.index === root.parentSliceIndex && root.activeSubTier !== "")
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

            // Material Symbol Icon
            MaterialSymbol {
                anchors.centerIn: parent
                text: sliceOverlay.sliceData ? sliceOverlay.sliceData.icon : ""
                iconSize: (sliceOverlay.isHovered || sliceOverlay.isParentOfSub) ? 30 : 25
                fill: (sliceOverlay.isHovered || sliceOverlay.isParentOfSub) ? 1 : 0
                color: (sliceOverlay.isHovered || sliceOverlay.isParentOfSub)
                    ? root.colOnPrimary
                    : Qt.rgba(1.0, 1.0, 1.0, 0.95)

                Behavior on iconSize {
                    NumberAnimation {
                        duration: 120
                        easing.type: Easing.OutBack
                        easing.overshoot: 1.35
                    }
                }
                Behavior on color { ColorAnimation { duration: 90 } }
                Behavior on fill { NumberAnimation { duration: 90 } }
            }
        }
    }

    // ── 2. Localized Symmetrical Sub-Arc Icons Overlay ────────────────────────
    Repeater {
        model: root.subSliceCount

        delegate: Item {
            id: outerSliceOverlay
            required property int index

            readonly property real itemStartDeg: root.subStartAngle + outerSliceOverlay.index * root.subSliceWidth
            readonly property real itemEndDeg: itemStartDeg + root.subSliceWidth
            readonly property real midRad: ((itemStartDeg + itemEndDeg) / 2) * Math.PI / 180

            readonly property bool isOuterHovered: outerSliceOverlay.index === root.outerHoveredIndex
            readonly property real rLift: isOuterHovered ? 5.0 * root.outerHoverFactor : 0.0
            readonly property real curIconRadius: root.subIconRadius + rLift / 2

            readonly property real rawIconX: root.cx + curIconRadius * Math.cos(midRad)
            readonly property real rawIconY: root.cy + curIconRadius * Math.sin(midRad)

            readonly property var sliceData: root.subSlices && root.subSlices[outerSliceOverlay.index] ? root.subSlices[outerSliceOverlay.index] : null

            // Staggered reveal progress
            readonly property real sliceProgress: Math.min(Math.max(root.subRevealProgress - outerSliceOverlay.index, 0.0), 1.0)
            readonly property real sliceScale: (sliceProgress >= 1.0) ? 1.0 : (1.0 - Math.pow(1.0 - sliceProgress, 3))

            visible: (root.activeSubTier !== "" || subCollapseAnim.running) && sliceProgress > 0.0
            opacity: sliceProgress
            scale: sliceScale

            width: 40
            height: 40
            x: Math.round(rawIconX - width / 2)
            y: Math.round(rawIconY - height / 2)

            Behavior on x { NumberAnimation { duration: 110; easing.type: Easing.OutQuad } }
            Behavior on y { NumberAnimation { duration: 110; easing.type: Easing.OutQuad } }

            // Material Symbol Icon
            MaterialSymbol {
                anchors.centerIn: parent
                text: outerSliceOverlay.sliceData ? outerSliceOverlay.sliceData.icon : ""
                iconSize: outerSliceOverlay.isOuterHovered ? 26 : 22
                fill: outerSliceOverlay.isOuterHovered ? 1 : 0
                color: outerSliceOverlay.isOuterHovered
                    ? root.colOnPrimary
                    : Qt.rgba(1.0, 1.0, 1.0, 0.95)

                Behavior on iconSize {
                    NumberAnimation {
                        duration: 120
                        easing.type: Easing.OutBack
                        easing.overshoot: 1.35
                    }
                }
                Behavior on color { ColorAnimation { duration: 90 } }
                Behavior on fill { NumberAnimation { duration: 90 } }
            }
        }
    }

    // ── Floating Center Hub Button (Dynamic Function Name / Close 'X') ────────
    Rectangle {
        id: centerBtn
        anchors.centerIn: parent
        width: root.innerRadius * 2
        height: root.innerRadius * 2
        radius: width / 2
        color: root.centerHovered
            ? Qt.rgba(1.0, 1.0, 1.0, 0.22)
            : (root.activeHoverLabel !== ""
                ? Qt.rgba(0.08, 0.08, 0.12, 0.65)
                : Qt.rgba(0.06, 0.06, 0.08, 0.45))
        border.color: root.activeHoverLabel !== ""
            ? Qt.lighter(root.colPrimary, 1.25)
            : Qt.rgba(1.0, 1.0, 1.0, 0.22)
        border.width: root.activeHoverLabel !== "" ? 1.8 : 1.5

        scale: (root.hubScale >= 1.0) ? (root.centerHovered ? 1.06 : 1.0) : root.hubScale
        opacity: root.hubScale

        Behavior on color { ColorAnimation { duration: 80 } }
        Behavior on border.color { ColorAnimation { duration: 80 } }

        // 1. Close 'X' Icon (shown when no slice is hovered)
        MaterialSymbol {
            anchors.centerIn: parent
            text: "close"
            iconSize: root.centerHovered ? 24 : 22
            color: Qt.rgba(1.0, 1.0, 1.0, 0.95)
            visible: root.activeHoverLabel === ""
            opacity: root.activeHoverLabel === "" ? 1.0 : 0.0
            Behavior on opacity { NumberAnimation { duration: 80 } }
            Behavior on iconSize { NumberAnimation { duration: 60 } }
        }

        // 2. Dynamic Segment / Function Name (shown when hovering any slice)
        StyledText {
            anchors.centerIn: parent
            width: parent.width - 12
            text: root.activeHoverLabel
            font.pixelSize: ((typeof Appearance !== 'undefined' && Appearance.font && Appearance.font.pixelSize) ? Appearance.font.pixelSize.small : 13)
            font.weight: Font.DemiBold
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: root.colPrimary
            visible: root.activeHoverLabel !== ""
            opacity: root.activeHoverLabel !== "" ? 1.0 : 0.0
            Behavior on opacity { NumberAnimation { duration: 80 } }
        }
    }

    // ── 5. Configuration Synchronization ────────────────────────────────────
    Connections {
        target: RadialMenuActions
        function onConfigChanged() {
            root.currentSlices = RadialMenuActions.getSlicesFor("main", root.activeContext, root.contextWindow)
            if (root.activeSubTier !== "") {
                root.subSlices = RadialMenuActions.getSlicesFor(root.activeSubTier, root.activeContext, root.contextWindow)
            }
            pieCanvas.requestPaint()
        }
    }

    // ── 6. In-Menu Function Swap & File Target Customizer Modal ──────────────
    RadialMenuCustomizer {
        id: customizer
        z: 100
    }
}
