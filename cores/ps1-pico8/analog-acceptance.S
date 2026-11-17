# Original PS1 serial-pad diagnostic. No SDK, BIOS or game assets.
# Frame rows encode bytes as eight white/black tiles, least-significant bit first:
# A6 59, raw poll reply (9 bytes), received length, poll counter LE, XOR checksum.
# Digital replies have five bytes; unreceived bytes are zero, never invented axes.
.set noreorder
.set noat
.text
    # Stop any firmware pad service before taking exclusive SIO ownership.
    li $t1, 0x14
    jal 0xb0
    nop
    mfc0 $t0, $12
    li $t1, -2
    and $t0, $t0, $t1
    mtc0 $t0, $12
    nop
    lui $s0, 0x1f80
    lui $s1, 0x8001
    ori $s1, $s1, 0x4000
    lui $s3, 0x8001
    ori $s3, $s3, 0x1000
    move $s2, $zero
    sb $zero, 3($s1)
    li $t0, 0xa6
    sb $t0, 0($s1)
    li $t0, 0x59
    sb $t0, 1($s1)
    li $t0, 0x40
    sh $t0, 0x104a($s0)
    li $t0, 0x0d
    sh $t0, 0x1048($s0)
    li $t0, 0x88
    sh $t0, 0x104e($s0)

    # 320x240 display, no BIOS polling or memory-card writes.
    lui $t0, 0x0300
    sw $t0, 0x1814($s0)
    lui $t0, 0x0800
    ori $t0, $t0, 1
    sw $t0, 0x1814($s0)
    lui $t0, 0x0500
    sw $t0, 0x1814($s0)
    lui $t0, 0x06c6
    ori $t0, $t0, 0x0260
    sw $t0, 0x1814($s0)
    lui $t0, 0x0704
    ori $t0, $t0, 0x2018
    sw $t0, 0x1814($s0)
    lui $t0, 0x0200
    sw $t0, 0x1810($s0)
    sw $zero, 0x1810($s0)
    lui $t0, 0x00f0
    ori $t0, $t0, 0x0140
    sw $t0, 0x1810($s0)
loop:
    lbu $t0, 3($s1)
    li $t1, 0x73
    beq $t0, $t1, poll
    nop
    # The guest requests analog exactly as an analog-capable game would.
    # A Digital controller must reject it and continue answering 0x41.
    move $a0, $s3
    jal 0x80010800
    nop
    addiu $a0, $s3, 9
    jal 0x80010800
    nop
    addiu $a0, $s3, 18
    jal 0x80010800
    nop
poll:
    sb $zero, 2($s1)
    sb $zero, 3($s1)
    sh $zero, 4($s1)
    sh $zero, 6($s1)
    sw $zero, 8($s1)
    addiu $a0, $s3, 27
    jal 0x80010800
    nop
    sb $v0, 11($s1)
    lbu $t0, 4($s1)
    li $t1, 0x5a
    bne $t0, $t1, draw
    nop
    lbu $t0, 3($s1)
    li $t1, 0x41
    beq $t0, $t1, complete
    li $t1, 0x73
    bne $t0, $t1, draw
    nop
complete:
    addiu $s2, $s2, 1
draw:
    sb $s2, 12($s1)
    srl $t0, $s2, 8
    sb $t0, 13($s1)
    move $t0, $zero
    move $t1, $zero
checksum:
    addu $t2, $s1, $t0
    lbu $t2, 0($t2)
    addiu $t0, $t0, 1
    xor $t1, $t1, $t2
    slti $t2, $t0, 14
    bnez $t2, checksum
    nop
    sb $t1, 14($s1)

    # Pace complete reports at VBlank; checksum rejects partial raster updates.
    li $t0, -2
    sh $t0, 0x1070($s0)
vblank:
    lhu $t0, 0x1070($s0)
    nop
    andi $t0, $t0, 1
    beqz $t0, vblank
    nop
    move $s4, $zero
row:
    addu $t0, $s1, $s4
    lbu $s6, 0($t0)
    move $s5, $zero
bit:
    andi $t0, $s6, 1
    beqz $t0, black
    lui $t1, 0x0200
    lui $t1, 0x02ff
    ori $t1, $t1, 0xffff
black:
    sw $t1, 0x1810($s0)
    # Keep every row inside Swan's default 224-line overscan crop.
    sll $t0, $s4, 3
    addiu $t0, $t0, 48
    sll $t0, $t0, 16
    sll $t1, $s5, 5
    addiu $t1, $t1, 16
    or $t0, $t0, $t1
    sw $t0, 0x1810($s0)
    lui $t0, 0x0008
    ori $t0, $t0, 16
    sw $t0, 0x1810($s0)
    srl $s6, $s6, 1
    addiu $s5, $s5, 1
    slti $t0, $s5, 8
    bnez $t0, bit
    nop
    addiu $s4, $s4, 1
    slti $t0, $s4, 15
    bnez $t0, row
    nop
    b loop
    nop

# Fixed call addresses avoid unresolved MIPS relocations in the raw binary.
# a0: nine outgoing bytes; v0: actual received length, including final non-ACK byte.
.org 0x800
transaction:
    move $s7, $ra
    move $t4, $a0
    addiu $t5, $s1, 2
    move $t6, $zero
    sh $zero, 0x104a($s0)
    li $t0, 200
select_delay:
    addiu $t0, $t0, -1
    bnez $t0, select_delay
    nop
    li $t0, 0x1013
    sh $t0, 0x104a($s0)
next_byte:
    lbu $a0, 0($t4)
    jal 0x80010a00
    nop
    sb $v0, 0($t5)
    addiu $t4, $t4, 1
    addiu $t5, $t5, 1
    addiu $t6, $t6, 1
    li $t0, 0x1013
    sh $t0, 0x104a($s0)
    beqz $v1, end_transaction
    nop
    slti $t0, $t6, 9
    bnez $t0, next_byte
    nop
end_transaction:
    sh $zero, 0x104a($s0)
    move $v0, $t6
    jr $s7
    nop

# Bounded byte exchange; never replace a missing serial response with test data.
.org 0xa00
exchange:
    sb $a0, 0x1040($s0)
    li $t0, 200000
rx_wait:
    lhu $t1, 0x1044($s0)
    nop
    andi $t1, $t1, 2
    bnez $t1, received
    nop
    addiu $t0, $t0, -1
    bnez $t0, rx_wait
    nop
    li $v0, 0xff
    jr $ra
    move $v1, $zero
received:
    lbu $v0, 0x1040($s0)
    li $t0, 2000
    move $v1, $zero
ack_wait:
    lhu $t1, 0x1044($s0)
    nop
    # IRQ is latched; reading STAT can consume Swan's transient ACKINPUT bit.
    andi $t1, $t1, 0x200
    bnez $t1, acked
    nop
    addiu $t0, $t0, -1
    bnez $t0, ack_wait
    nop
    jr $ra
    nop
acked:
    jr $ra
    li $v1, 1
.org 0x1000
    .byte 1, 0x43, 0, 1, 0, 0, 0, 0, 0
    .byte 1, 0x44, 0, 1, 0, 0, 0, 0, 0
    .byte 1, 0x43, 0, 0, 0, 0, 0, 0, 0
    .byte 1, 0x42, 0, 0, 0, 0, 0, 0, 0
