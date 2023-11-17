import gdb
from dataclasses import dataclass

SIZE_CLASS_COUNT = 25


@dataclass
class SizeClassMeta:
    size: int
    slab_order: int
    objects_per_slab: int


def read_u64(inferior: gdb.Inferior, addr: int) -> int:
    return int.from_bytes(inferior.read_memory(addr, 8), "little")


def count_int_bits(b: int):
    count = 0
    while b:
        count += 1
        b &= b - 1
    return count


def count_set_bits(bitmap: bytes) -> int:
    return sum(count_int_bits(b) for b in bitmap)


class HeapDumpCmd(gdb.Command):
    """Dumps current heap information"""

    def __init__(self):
        super(HeapDumpCmd, self).__init__("heapdump", gdb.COMMAND_USER)

    def complete(self, text, word):
        return gdb.COMPLETE_NONE

    def _print_slab_list(self, cur_slab_addr: int, metadata: SizeClassMeta):
        bitmap_size = (metadata.objects_per_slab + 7) // 8
        slab_header_type = gdb.lookup_type("kernel::mm::heap::SlabHeader")
        inferior = gdb.inferiors()[0]
        while cur_slab_addr != 0:
            if cur_slab_addr == 1:
                print("    (unlinked, HEAP CORRUPT)")
                break

            slab_ptr = gdb.Value(cur_slab_addr).cast(slab_header_type.pointer())
            slab = slab_ptr.dereference()
            allocated_objs = int(slab["allocated"]["value"]["value"])
            bitmap_ptr = int(slab_ptr + 1)
            bitmap = inferior.read_memory(bitmap_ptr, bitmap_size).tobytes()
            bitmap_allocated_objs = count_set_bits(bitmap)
            print(
                f"    Slab {cur_slab_addr:#x} ({allocated_objs} allocated, {bitmap_allocated_objs} allocated in bitmap)"
            )
            if allocated_objs != bitmap_allocated_objs:
                print("        (allocated count and bitmap disagree, HEAP CORRUPT)")
            cur_slab_addr = read_u64(inferior, int(slab["link"]["next"].address))

    def invoke(self, args, from_tty):
        size_class_type = gdb.lookup_type("kernel::mm::heap::SizeClass")
        allocator_addr = (
            gdb.lookup_static_symbol("kernel::mm::heap::ALLOCATOR").value().address
        ).cast(size_class_type.pointer())

        inferior = gdb.inferiors()[0]
        for size_class_idx in range(SIZE_CLASS_COUNT):
            size_class = allocator_addr[size_class_idx]
            metadata = size_class["meta"]
            inner = size_class["inner"]["data"]["value"]
            metadata = SizeClassMeta(
                int(metadata["size"]),
                int(metadata["slab_order"]),
                int(metadata["objects_per_slab"]),
            )
            partial_slab_head_addr = int(inner["partial_slabs"]["head"].address)
            partial_slab_head = read_u64(inferior, partial_slab_head_addr)

            print(f"Size class {metadata.size} (slab order {metadata.slab_order}):")
            self._print_slab_list(partial_slab_head, metadata)
            print()


HeapDumpCmd()
