"""Small, dependency-free icon renderer for the desktop app.

The GUI only needs a handful of simple line icons. Drawing them with QtGui keeps
PySide6-Addons (and its QtSvg/QML/Quick/PDF libraries) out of standalone builds.
"""

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPainterPath, QPen, QPixmap


APP_ICON_SVG = "app"
DATABASE_REFRESH_SVG = "database-refresh"
FILE_ICON_SVG = "file"
SUCCESS_ICON_SVG = "success"
WARNING_ICON_SVG = "warning"
CLOSE_ICON_SVG = "close"
FOLDER_ICON_SVG = "folder"
INFO_ICON_SVG = "info"
ARROW_RIGHT_SVG = "arrow-right"

_ICON_COLORS = {
    APP_ICON_SVG: "#2563EB",
    DATABASE_REFRESH_SVG: "#2563EB",
    FILE_ICON_SVG: "#4B5563",
    SUCCESS_ICON_SVG: "#16A34A",
    WARNING_ICON_SVG: "#DC2626",
    CLOSE_ICON_SVG: "#9CA3AF",
    FOLDER_ICON_SVG: "#3B82F6",
    INFO_ICON_SVG: "#2563EB",
    ARROW_RIGHT_SVG: "#4B5563",
}


def _path(points: list[tuple[float, float]], scale: float) -> QPainterPath:
    path = QPainterPath(QPointF(points[0][0] * scale, points[0][1] * scale))
    for x, y in points[1:]:
        path.lineTo(x * scale, y * scale)
    return path


def get_icon_from_svg(icon_name: str, size: int = 24) -> QIcon:
    """Return a crisp line icon drawn with QtGui only.

    The function name is retained for compatibility with the original GUI
    call sites, although icons are now identified by a small string constant.
    """
    scale = size / 24
    pixmap = QPixmap(size, size)
    pixmap.fill(Qt.GlobalColor.transparent)

    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing)
    pen = QPen(QColor(_ICON_COLORS.get(icon_name, "#4B5563")), max(1.25, 2 * scale))
    pen.setCapStyle(Qt.PenCapStyle.RoundCap)
    pen.setJoinStyle(Qt.PenJoinStyle.RoundJoin)
    painter.setPen(pen)
    painter.setBrush(Qt.BrushStyle.NoBrush)

    if icon_name in (APP_ICON_SVG, DATABASE_REFRESH_SVG):
        painter.drawEllipse(QRectF(3 * scale, 2 * scale, 18 * scale, 6 * scale))
        body = QPainterPath(QPointF(3 * scale, 5 * scale))
        body.lineTo(3 * scale, 19 * scale)
        body.cubicTo(3 * scale, 23 * scale, 21 * scale, 23 * scale, 21 * scale, 19 * scale)
        body.lineTo(21 * scale, 5 * scale)
        painter.drawPath(body)
        middle = QPainterPath(QPointF(3 * scale, 12 * scale))
        middle.cubicTo(3 * scale, 16 * scale, 21 * scale, 16 * scale, 21 * scale, 12 * scale)
        painter.drawPath(middle)
    elif icon_name == FILE_ICON_SVG:
        outline = _path([(6, 2), (14, 2), (20, 8), (20, 22), (6, 22), (4, 20), (4, 4), (6, 2)], scale)
        painter.drawPath(outline)
        painter.drawPath(_path([(14, 2), (14, 8), (20, 8)], scale))
    elif icon_name == SUCCESS_ICON_SVG:
        painter.drawEllipse(QRectF(2 * scale, 2 * scale, 20 * scale, 20 * scale))
        painter.drawPath(_path([(7, 12), (10.5, 15.5), (17.5, 8.5)], scale))
    elif icon_name == WARNING_ICON_SVG:
        triangle = _path([(12, 3), (22, 21), (2, 21), (12, 3)], scale)
        painter.drawPath(triangle)
        painter.drawLine(QPointF(12 * scale, 9 * scale), QPointF(12 * scale, 14 * scale))
        painter.drawPoint(QPointF(12 * scale, 17.5 * scale))
    elif icon_name == CLOSE_ICON_SVG:
        painter.drawLine(QPointF(6 * scale, 6 * scale), QPointF(18 * scale, 18 * scale))
        painter.drawLine(QPointF(18 * scale, 6 * scale), QPointF(6 * scale, 18 * scale))
    elif icon_name == FOLDER_ICON_SVG:
        folder = _path([(2, 6), (9, 6), (11, 9), (22, 9), (22, 20), (2, 20), (2, 6)], scale)
        painter.drawPath(folder)
    elif icon_name == INFO_ICON_SVG:
        painter.drawEllipse(QRectF(2 * scale, 2 * scale, 20 * scale, 20 * scale))
        painter.drawLine(QPointF(12 * scale, 11 * scale), QPointF(12 * scale, 17 * scale))
        painter.drawPoint(QPointF(12 * scale, 7.5 * scale))
    elif icon_name == ARROW_RIGHT_SVG:
        painter.drawLine(QPointF(5 * scale, 12 * scale), QPointF(19 * scale, 12 * scale))
        painter.drawPath(_path([(13, 6), (19, 12), (13, 18)], scale))

    painter.end()
    return QIcon(pixmap)
