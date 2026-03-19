    .section .text.m_entry
    .globl _m_start
_m_start:
    la   sp, _m_stack_top
    csrw mscratch, sp

    li   t0, (1 << 11) | (1 << 7)
    csrw mstatus, t0

    la   t0, _start
    csrw mepc, t0

    la   t0, _m_trap
    csrw mtvec, t0

    li   t0, 0xffff
    csrw mideleg, t0

    li   t0, 0xffff
    li   t1, (1 << 9)
    not  t1, t1
    and  t0, t0, t1
    csrw medeleg, t0

    li   t0, -1
    csrw pmpaddr0, t0
    li   t0, 0x0f
    csrw pmpcfg0, t0

    li   t0, -1
    csrw mcounteren, t0

    li   t0, 0x0200bff8
    ld   t1, 0(t0)
    li   t2, 12500000
    add  t1, t1, t2
    li   t0, 0x02004000
    sd   t1, 0(t0)

    li   t0, (1 << 7)
    csrw mie, t0

    mret

    .section .bss.m_stack
    .align 12
    .space 16384
    .globl _m_stack_top
_m_stack_top:

    .section .text.m_trap
    .align 2
    .globl _m_trap
_m_trap:
    csrrw sp, mscratch, sp
    addi  sp, sp, -128

    sd    ra,  0(sp)
    sd    t0,  8(sp)
    sd    t1, 16(sp)
    sd    t2, 24(sp)
    sd    t3, 32(sp)
    sd    a0, 40(sp)
    sd    a1, 48(sp)
    sd    a2, 56(sp)
    sd    a3, 64(sp)
    sd    a4, 72(sp)
    sd    a5, 80(sp)
    sd    a6, 88(sp)
    sd    a7, 96(sp)

    csrr  t0, mcause
    bgez  t0, _m_trap_ecall

    slli  t1, t0, 1
    srli  t1, t1, 1

    li    t2, 7
    bne   t1, t2, _m_trap_done

    li    t0, 0x02004000
    ld    t1, 0(t0)
    li    t2, 12500000
    add   t1, t1, t2
    sd    t1, 0(t0)

    li    t0, (1 << 5)
    csrs  mip, t0
    j     _m_trap_done

_m_trap_ecall:
    li    t1, 9
    bne   t0, t1, _m_trap_done

    li    t1, 1
    beq   a7, t1, _m_sbi_putchar

    li    t1, 8
    beq   a7, t1, _m_sbi_shutdown

    li    t1, 0x53525354
    beq   a7, t1, _m_sbi_shutdown

    li    t1, 0x54494d45
    beq   a7, t1, _m_sbi_set_timer

    li    a0, -2
    j     _m_trap_ecall_done

_m_sbi_putchar:
    li    t0, 0x10000000
    sb    a0, 0(t0)
    li    a0, 0
    j     _m_trap_ecall_done

_m_sbi_shutdown:
    li    t0, 0x100000
    li    t1, 0x5555
    sw    t1, 0(t0)
1:  j     1b

_m_sbi_set_timer:
    li    t0, 0x02004000
    sd    a0, 0(t0)
    li    t0, (1 << 5)
    csrc  mip, t0
    li    a0, 0
    j     _m_trap_ecall_done

_m_trap_ecall_done:
    csrr  t0, mepc
    addi  t0, t0, 4
    csrw  mepc, t0

_m_trap_done:
    ld    ra,  0(sp)
    ld    t0,  8(sp)
    ld    t1, 16(sp)
    ld    t2, 24(sp)
    ld    t3, 32(sp)
    ld    a2, 56(sp)
    ld    a3, 64(sp)
    ld    a4, 72(sp)
    ld    a5, 80(sp)
    ld    a6, 88(sp)
    ld    a7, 96(sp)
    addi  sp, sp, 128
    csrrw sp, mscratch, sp
    mret
