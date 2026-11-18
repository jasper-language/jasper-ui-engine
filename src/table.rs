//! Host-neutral table measurement derived from the actual native layout.
//! 从真实原生布局导出的宿主中立表格测量结果。
use crate::nostd_prelude::*;
use crate::geometry::Rect;
use crate::scene::NodeId;

/// One measured table cell. `row` and `column` are zero-based positions in source row/cell order.
/// 一个已测量表格单元；`row`/`column` 为源行/单元顺序中的从零索引。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableCellMetrics {
    pub node: NodeId,
    pub row: usize,
    pub column: usize,
    pub rect: Rect,
}

/// Measured column/row extents and every visible cell of one table subtree.
/// 一个表格子树测得的列/行尺寸与所有可见单元。
#[derive(Clone, Debug, PartialEq)]
pub struct TableMetrics {
    pub table: NodeId,
    pub rect: Rect,
    pub column_widths: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub cells: Vec<TableCellMetrics>,
}
