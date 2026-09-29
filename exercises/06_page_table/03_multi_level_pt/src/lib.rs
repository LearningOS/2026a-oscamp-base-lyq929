//! # SV39 三级页表
use std::collections::HashMap;
/// 页大小 4KB
pub const PAGE_SIZE: usize = 4096;
/// 每级页表有 512 个条目 (2^9)
pub const PT_ENTRIES: usize = 512;
/// PTE 标志位
pub const PTE_V: u64 = 1 << 0; // Valid
pub const PTE_R: u64 = 1 << 1; // Readable
pub const PTE_W: u64 = 1 << 2; // Writable
pub const PTE_X: u64 = 1 << 3; // Executable
/// PPN 在 PTE 中的偏移
const PPN_SHIFT: u32 = 10;
/// 页表节点：一个包含 512 个条目的数组
#[derive(Clone)]
pub struct PageTableNode {
    pub entries: [u64; PT_ENTRIES],
}
impl PageTableNode {
    pub fn new() -> Self {
        Self {
            entries: [0; PT_ENTRIES],
        }
    }
}
impl Default for PageTableNode {
    fn default() -> Self {
        Self::new()
    }
}
/// 模拟的三级页表。
pub struct Sv39PageTable {
    /// 物理页号 -> 页表节点
    nodes: HashMap<u64, PageTableNode>,
    /// 根页表的物理页号
    pub root_ppn: u64,
    /// 下一个可分配的物理页号（简易分配器）
    next_ppn: u64,
}
/// 翻译结果
#[derive(Debug, PartialEq)]
pub enum TranslateResult {
    Ok(u64),
    PageFault,
}
impl Sv39PageTable {
    pub fn new() -> Self {
        let mut pt = Self {
            nodes: HashMap::new(),
            root_ppn: 0x80000,
            next_ppn: 0x80001,
        };
        pt.nodes.insert(pt.root_ppn, PageTableNode::new());
        pt
    }
    /// 分配一个新的物理页并初始化为空页表节点，返回其 PPN。
    fn alloc_node(&mut self) -> u64 {
        let ppn = self.next_ppn;
        self.next_ppn += 1;
        self.nodes.insert(ppn, PageTableNode::new());
        ppn
    }
    /// 从 39 位虚拟地址中提取第 `level` 级的 VPN。
    ///
    /// - `level=2`: 取 bits [38:30]
    /// - `level=1`: 取 bits [29:21]
    /// - `level=0`: 取 bits [20:12]
    pub fn extract_vpn(va: u64, level: usize) -> usize {
        let shift = 12 + level * 9;
        ((va >> shift) & 0x1FF) as usize
    }
    /// 建立从虚拟页到物理页的映射（4KB 页）。
    pub fn map_page(&mut self, va: u64, pa: u64, flags: u64) {
        // 对齐到4KB页边界
        let va_page = va & !(PAGE_SIZE as u64 - 1);
        let pa_page = pa & !(PAGE_SIZE as u64 - 1);
        let target_ppn = pa_page >> 12;
        let mut curr_ppn = self.root_ppn;
        // 遍历 level 2, level1
        for level in [2, 1] {
            let vpn = Self::extract_vpn(va_page, level);
            let pte = self.nodes.get(&curr_ppn).unwrap().entries[vpn];
            if (pte & PTE_V) == 0 {
                let child_ppn = self.alloc_node();
                let node = self.nodes.get_mut(&curr_ppn).unwrap();
                node.entries[vpn] = (child_ppn << PPN_SHIFT) | PTE_V;
            }
            let pte = self.nodes.get(&curr_ppn).unwrap().entries[vpn];
            curr_ppn = pte >> PPN_SHIFT;
        }
        // level 0，写入叶子PTE
        let vpn0 = Self::extract_vpn(va_page, 0);
        let node0 = self.nodes.get_mut(&curr_ppn).unwrap();
        node0.entries[vpn0] = (target_ppn << PPN_SHIFT) | flags;
    }
    /// 遍历三级页表，将虚拟地址翻译为物理地址。
    pub fn translate(&self, va: u64) -> TranslateResult {
        let mut curr_ppn = self.root_ppn;
        let levels = [2, 1, 0];
        for &level in &levels {
            let vpn = Self::extract_vpn(va, level);
            let node = match self.nodes.get(&curr_ppn) {
                Some(n) => n,
                None => return TranslateResult::PageFault,
            };
            let pte = node.entries[vpn];
            if (pte & PTE_V) == 0 {
                return TranslateResult::PageFault;
            }
            // 判断是否叶子PTE：R/W/X任意一个置位
            let is_leaf = (pte & (PTE_R | PTE_W | PTE_X)) != 0;
            if is_leaf {
                let ppn = pte >> PPN_SHIFT;
                let pa = match level {
                    2 => {
                        // Level2 1GB大页，保留va低30位
                        (ppn << 12) | (va & ((1u64 << 30) - 1))
                    }
                    1 => {
                        // Level1 2MB大页，保留va低21位
                        (ppn << 12) | (va & ((1u64 << 21) - 1))
                    }
                    0 => {
                        // Level0 4KB页，保留va低12位offset
                        (ppn << 12) | (va & 0xFFF)
                    }
                    _ => unreachable!(),
                };
                return TranslateResult::Ok(pa);
            }
            // 非叶子，取下一级页表PPN
            curr_ppn = pte >> PPN_SHIFT;
        }
        TranslateResult::PageFault
    }
    /// 建立大页映射（2MB superpage，在 level 1 设叶子 PTE）。
    pub fn map_superpage(&mut self, va: u64, pa: u64, flags: u64) {
        let mega_size: u64 = (PAGE_SIZE * PT_ENTRIES) as u64; // 2MB
        assert_eq!(va % mega_size, 0, "va must be 2MB-aligned");
        assert_eq!(pa % mega_size, 0, "pa must be 2MB-aligned");
        let target_ppn = pa >> 12;
        let mut curr_ppn = self.root_ppn;
        // 只遍历 level 2
        let level2 = 2;
        let vpn2 = Self::extract_vpn(va, level2);
        let pte = self.nodes.get(&curr_ppn).unwrap().entries[vpn2];
        if (pte & PTE_V) == 0 {
            let child_ppn = self.alloc_node();
            let node_root = self.nodes.get_mut(&curr_ppn).unwrap();
            node_root.entries[vpn2] = (child_ppn << PPN_SHIFT) | PTE_V;
        }
        let pte = self.nodes.get(&curr_ppn).unwrap().entries[vpn2];
        curr_ppn = pte >> PPN_SHIFT;
        // level1 写入叶子PTE（大页）
        let vpn1 = Self::extract_vpn(va, 1);
        let node_l1 = self.nodes.get_mut(&curr_ppn).unwrap();
        node_l1.entries[vpn1] = (target_ppn << PPN_SHIFT) | flags;
    }
}
impl Default for Sv39PageTable {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_extract_vpn() {
        let va: u64 = 0x7FFFFFF000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 0x1FF);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0x1FF);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 0x1FF);
    }
    #[test]
    fn test_extract_vpn_simple() {
        let va: u64 = 0x1000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 1);
    }
    #[test]
    fn test_extract_vpn_level2() {
        let va: u64 = 0x40000000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 1);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 0);
    }
    #[test]
    fn test_map_and_translate_single() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x1000, 0x80001000, PTE_V | PTE_R);
        let result = pt.translate(0x1000);
        assert_eq!(result, TranslateResult::Ok(0x80001000));
    }
    #[test]
    fn test_translate_with_offset() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x2000, 0x90000000, PTE_V | PTE_R | PTE_W);
        let result = pt.translate(0x2ABC);
        assert_eq!(result, TranslateResult::Ok(0x90000ABC));
    }
    #[test]
    fn test_translate_page_fault() {
        let pt = Sv39PageTable::new();
        assert_eq!(pt.translate(0x1000), TranslateResult::PageFault);
    }
    #[test]
    fn test_multiple_mappings() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x0000_1000, 0x8000_1000, PTE_V | PTE_R);
        pt.map_page(0x0000_2000, 0x8000_5000, PTE_V | PTE_R | PTE_W);
        pt.map_page(0x0040_0000, 0x9000_0000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1234), TranslateResult::Ok(0x80001234));
        assert_eq!(pt.translate(0x2000), TranslateResult::Ok(0x80005000));
        assert_eq!(pt.translate(0x400100), TranslateResult::Ok(0x90000100));
    }
    #[test]
    fn test_map_overwrite() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x1000, 0x80001000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1000), TranslateResult::Ok(0x80001000));
        pt.map_page(0x1000, 0x90002000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1000), TranslateResult::Ok(0x90002000));
    }
    #[test]
    fn test_superpage_mapping() {
        let mut pt = Sv39PageTable::new();
        pt.map_superpage(0x200000, 0x80200000, PTE_V | PTE_R | PTE_W);
        assert_eq!(pt.translate(0x200000), TranslateResult::Ok(0x80200000));
        assert_eq!(pt.translate(0x200ABC), TranslateResult::Ok(0x80200ABC));
        assert_eq!(pt.translate(0x2FF000), TranslateResult::Ok(0x802FF000));
    }
    #[test]
    fn test_superpage_and_normal_coexist() {
        let mut pt = Sv39PageTable::new();
        pt.map_superpage(0x0, 0x80000000, PTE_V | PTE_R);
        pt.map_page(0x40000000, 0x90001000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x100), TranslateResult::Ok(0x80000100));
        assert_eq!(pt.translate(0x40000000), TranslateResult::Ok(0x90001000));
    }
}
