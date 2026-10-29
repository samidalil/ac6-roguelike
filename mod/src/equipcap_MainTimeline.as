package _02_010_EquipTop_fla
{
   import flash.display.MovieClip;
   import flash.display.Shape;

   // menu/02_010_equipcap.gfx: the vanilla part list window (02_010_EquipTop) as the inventory cap opens it (VENTE when
   // the inventory is over the cap). The 3D AC view (PlayerModelView) and the AC specs slot (DetailStatusView) are
   // removed from the timeline (tools/build_equipcap.py); this class adds an opaque backdrop under everything: part
   // categories, the list, the part's description and its stats only.
   [SWF(width="1920", height="1080", backgroundColor="#787878", frameRate="30")]
   public dynamic class MainTimeline extends MovieClip
   {

      public var MenuTitle:MovieClip;

      public var StatusBar:MovieClip;

      public var ItemList:MovieClip;

      public var AdditionalProperties:MovieClip;

      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1,8,this.frame9,16,this.frame17,24,this.frame25,31,this.frame32);
         var backdrop:Shape = new Shape();
         backdrop.graphics.beginFill(0x0B1016, 0.96);
         backdrop.graphics.drawRect(-400, -300, 2720, 1680);
         backdrop.graphics.endFill();
         this.addChildAt(backdrop, 0);
      }

      internal function frame1() : *
      {
         stop();
      }

      internal function frame9() : *
      {
         gotoAndPlay("Loop");
      }

      internal function frame17() : *
      {
         stop();
      }

      internal function frame25() : *
      {
         gotoAndPlay("Loop");
      }

      internal function frame32() : *
      {
         stop();
      }
   }
}
