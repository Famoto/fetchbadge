// Velcro-backed case for the LilyGO T5 4.7" e-paper S3 (V2.3).
// Two parts: tray (back, velcro holes) + bezel (front, display window).
// Stack inside, front to back: clear lens -> panel+PCB -> LiPo pouch.
// Board numbers come from LilyGO's EPD47-S3.dxf; MEASURE YOUR BOARD and tweak.
//
// Render:  openscad -D 'part="tray"'  -o tray.stl  nametag_case.scad
//          openscad -D 'part="bezel"' -o bezel.stl nametag_case.scad

part = "both";          // "tray" | "bezel" | "both"

// --- board (from DXF: PCB 118.1 x 63.1, panel glass 0.87 thick) ---
pcb_w = 118.1;
pcb_h = 63.1;
fit   = 0.45;           // clearance per side (0.8 less than v2 so the tray ledge could grow to 2 mm)
front_stack = 2.5;      // panel + PCB thickness, front of glass to PCB back
back_parts  = 4.5;      // tallest part on the PCB back (USB-C, buttons, JST)
lens_t = 1.0;           // clear acrylic/PETG protector over the glass; 0 = none
bat_t  = 10.0;          // LiPo thickness (was 5 for a 503450; +5 mm for Luna's 2000 mAh pack)

// --- shell ---
wall  = 6.8;            // keeps outer at 132.6 x 77.6 with the smaller cavity
floor_t = 2;
bezel_t = 4;
// bezel frame width, outer edge to window edge (active area is 103.68 x 58.32)
frame_usb = 18;         // USB-C end (-X)
frame     = 9;          // the other three sides
win_dy = 0;

// --- side cutouts (USB-C on a short end, RST/IO0/IO21 on a long edge) ---
// USB-C sits 8 mm off centre TOWARD the button edge (checked against 3DMath's
// T5 4.7 S3 housing on MakerWorld).
mirror_ports = false;   // flip if the cutouts land on the wrong side
usb_y  = 8;             // USB-C centre offset from board centre, toward buttons
usb_w  = 12; usb_hgt = 7.5;
btn_from_usb_end = 27;  // RST/BOOT/IO21 sit 28.5..50.9 mm from the USB end
btn_len = 25; btn_hgt = 6;

// --- velcro: through-holes in the back to sew/zip-tie a strip on ---
velcro_w = 100; velcro_h = 50;   // hole pattern spans this
stitch_d = 3;

// M2.5x6 heat-set knurled inserts in the tray corners, M2.5 screws through the bezel
insert_d = 3.3;         // hole for a 3.5 mm OD insert; use your insert's recommended size
insert_l = 6;
screw_clear = 2.9;

// ledge around the tray's inner top edge
lip_w = 2.0; lip_h = 1.2;   // recess (tray ledge) width and depth

$fn = 48;

in_w = pcb_w + 2*fit;
in_h = pcb_h + 2*fit;
depth = lens_t + front_stack + back_parts + bat_t;
tray_h = floor_t + depth;
pcb_back_z = tray_h - lens_t - front_stack;
port_z = pcb_back_z - back_parts/2;
screws = [for (sx=[-1,1], sy=[-1,1]) [sx*(in_w/2+wall/2), sy*(in_h/2+wall/2)]];
s = mirror_ports ? -1 : 1;
out_w = in_w + 2*wall; out_h = in_h + 2*wall;
win_w = out_w - frame_usb - frame;
win_h = out_h - 2*frame;
win_dx = (frame_usb - frame)/2;   // shifts window away from the USB end

module outline() {
    offset(r=wall/2) square([in_w+wall, in_h+wall], center=true);
}

module tray() {
    difference() {
        linear_extrude(tray_h) outline();
        translate([0,0,floor_t]) linear_extrude(depth+1) square([in_w, in_h], center=true);
        // ledge (recess) around the top inner edge
        translate([0,0,tray_h-lip_h]) linear_extrude(lip_h+1)
            offset(delta=lip_w) square([in_w, in_h], center=true);
        // corner insert holes (+0.5 mm so melt has somewhere to go)
        for (p=screws) translate([p[0],p[1],tray_h-insert_l-0.5]) cylinder(d=insert_d, h=insert_l+1.5);
        // USB-C (short end, -X)
        translate([-in_w/2-wall-1, -s*usb_y-usb_w/2, port_z-usb_hgt/2])
            cube([wall+2, usb_w, usb_hgt]);
        // button slot (long edge, -Y)
        translate([-in_w/2+btn_from_usb_end, -s*(in_h/2+wall/2)-(wall+2)/2, port_z-btn_hgt/2])
            cube([btn_len, wall+2, btn_hgt]);
        // velcro holes: 3 per long side
        for (sx=[-1,0,1], sy=[-1,1])
            translate([sx*(velcro_w/2-4), sy*(velcro_h/2-4), -1]) cylinder(d=stitch_d, h=floor_t+2);
    }
}

module bezel() {
    difference() {
        linear_extrude(bezel_t) outline();
        translate([win_dx, win_dy, -1]) linear_extrude(bezel_t+2)
            offset(r=1) square([win_w-2, win_h-2], center=true);
        for (p=screws) translate([p[0],p[1],-1]) cylinder(d=screw_clear, h=bezel_t+2);
    }
}

if (part == "tray") tray();
if (part == "bezel") bezel();   // print face-down
if (part == "both") { tray(); translate([0,0,tray_h+10]) bezel(); }
