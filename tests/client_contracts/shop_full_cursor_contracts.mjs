import { assert } from './assertions.mjs';
import { KIND, ABILITY } from '../../client/src/protocol.js';
import { carriedItemSlotsFull, ShopFullCursor } from '../../client/src/input/shop_full_cursor.js';
import { _shopFullCursorTarget } from '../../client/src/input/commands.js';
const unit = {kind: KIND.WARRIOR, owner: 1, hp: 58, itemRadianceTier: 1};
const shop = {kind: KIND.SHOP, owner: 1, hp: 200};
const input = {mouse: {x: 50,y: 70}, state: {playerId: 1, selectedEntities: () => [shop]},
  clientIntent: {}, _entityAtScreen: () => unit, _shopFullCursorTarget};
assert(_shopFullCursorTarget.call(input) === unit, 'selected Shop shows full Novice');
for (const kind of [KIND.WARRIOR,KIND.WIND_ADEPT,KIND.WAR_ADEPT,KIND.MAGIC_ADEPT]) {
  const capacity = kind === KIND.WARRIOR ? 1 : 3;
  assert(carriedItemSlotsFull({...unit,kind,itemRadianceTier:capacity}), 'rank capacity shows hint');
  assert(!carriedItemSlotsFull({...unit,kind,itemRadianceTier:capacity-1}), 'free slot hides hint');
  assert(!carriedItemSlotsFull({...unit,kind,daoMaster:true,itemRadianceTier:5}), 'Master fifth item leaves room');
  assert(carriedItemSlotsFull({...unit,kind,daoMaster:true,itemRadianceTier:6}), 'Master sixth item fills slots');
}
assert(!carriedItemSlotsFull({...unit,kind:KIND.RIFLEMAN}), 'unlimited other units have no hint');
unit.owner = 2;
assert(!_shopFullCursorTarget.call(input), 'enemy receives no friendly hint');
input.state.isAllyOwner = owner => owner === 2;
assert(_shopFullCursorTarget.call(input) === unit, 'visible allied full carrier has hint');
unit.visionOnly = true;
assert(!_shopFullCursorTarget.call(input), 'vision-only records are excluded');
unit.visionOnly = false;
input.clientIntent.commandTarget = {kind:'ability',ability:ABILITY.REGEN};
assert(_shopFullCursorTarget.call(input), 'Regen uses a carried slot at capacity');
input.clientIntent.commandTarget.ability = ABILITY.HASTE;
assert(_shopFullCursorTarget.call(input), 'carried-item targeting retains hint');
const originalPick = input._entityAtScreen;
let overlapping = [shop, {...unit, owner: 3}, unit];
input._entityAtScreen = (_point, _ownPreferred, eligible = () => true) => overlapping.find(eligible);
assert(_shopFullCursorTarget.call(input) === unit, 'item hint picks through buildings and enemies like purchase targeting');
overlapping = [{...unit, itemRadianceTier: 0}, unit];
assert(!_shopFullCursorTarget.call(input), 'hint does not skip a friendly recipient with free slots');
input.clientIntent.commandTarget = null;
overlapping = [shop, unit];
assert(!_shopFullCursorTarget.call(input), 'ordinary hover still uses the frontmost entity');
input._entityAtScreen = originalPick;
input.inputRouter = {activePreviewSurface: () => 'hud'};
assert(!_shopFullCursorTarget.call(input), 'UI hover clears hint');
input.inputRouter = null;
input._drag = {};
assert(!_shopFullCursorTarget.call(input), 'drag clears hint');
input._drag = null;
input.state.selectedEntities = () => [];
assert(!_shopFullCursorTarget.call(input), 'deselection clears hint');
const before = globalThis.document;
let removed = false;
globalThis.document = {createElement: () => ({style:{},remove(){removed=true;}})};
try {
  const dom = {clientWidth:300,clientHeight:200,appendChild(){}};
  const hint = new ShopFullCursor(dom);
  hint.update({mouse:{x:50,y:70},_shopFullCursorTarget:()=>unit});
  assert(!hint.label.hidden && hint.label.style.transform==='translate(66px, 88px)', 'plain label follows gameplay cursor');
  hint.update({mouse:null,_shopFullCursorTarget:()=>unit});
  assert(hint.label.hidden, 'leaving viewport clears hint');
  hint.destroy();
  assert(removed, 'teardown removes label');
} finally {globalThis.document=before;}
console.log('Shop full cursor contracts passed');

// Shop hover previews follow recipient eligibility and the explicit intent seam.
const { updateShopDeliveryPreview } = await import('../../client/src/input/shop_delivery_preview.js');
const { buildRendererFeedbackView } = await import('../../client/src/renderer/feedback_view_model.js');
const previewShop = {id:91,kind:KIND.SHOP,owner:1,hp:200,x:100,y:200};
let hovered = {id:92,kind:KIND.WARRIOR,owner:1,hp:58,x:1500,y:200,itemRadianceTier:0};
const previewIntent = {updateShopDeliveryPreview(value) {this.shopDeliveryPreview=value;}};
const previewInput = {_intent:()=>previewIntent,mouse:{x:50,y:50},
  state:{playerId:1,selectedEntities:()=>[previewShop]},
  _entityAtScreen:(_point,_own,eligible)=>eligible(hovered)?hovered:null};
updateShopDeliveryPreview(previewInput);
assert(previewIntent.shopDeliveryPreview?.targetX===1500,'hover line has unlimited range');
assert(buildRendererFeedbackView(previewInput.state,{clientIntent:previewIntent}).shopDeliveryPreview?.x===100,
  'hover line reaches renderer feedback');
hovered={...hovered,owner:2};
updateShopDeliveryPreview(previewInput);
assert(!previewIntent.shopDeliveryPreview,'enemy hover clears line');
previewInput.state.isAllyOwner=owner=>owner===2;
updateShopDeliveryPreview(previewInput);
assert(previewIntent.shopDeliveryPreview,'allied recipients show line');
hovered={...hovered,itemRadianceTier:1};
updateShopDeliveryPreview(previewInput);
assert(!previewIntent.shopDeliveryPreview,'full carried inventory hides line');
previewIntent.commandTarget={kind:'ability',ability:ABILITY.REGEN};
updateShopDeliveryPreview(previewInput);
assert(!previewIntent.shopDeliveryPreview,'Regen is rejected at full inventory');
hovered={...hovered,visionOnly:true};
updateShopDeliveryPreview(previewInput);
assert(!previewIntent.shopDeliveryPreview,'vision-only entities never show line');

// The buy menu draws every selected cultivator, independent of hover or free slots.
previewIntent.commandTarget = null;
previewIntent.commandCardMode = "shopBuy";
previewInput.mouse = null;
const buyers = [
  {id:93, kind:KIND.WAR_ADEPT, owner:1, hp:176, x:300, y:220, itemRadianceTier:3},
  {id:94, kind:KIND.WARRIOR, owner:1, hp:58, x:400, y:240},
  {id:95, kind:KIND.WORKER, owner:1, hp:58, x:500, y:250},
  {id:96, kind:KIND.WARRIOR, owner:2, hp:58, x:600, y:260},
];
previewInput.state.selectedEntities = () => buyers;
let previewEntities = [previewShop, ...buyers];
previewInput.state.entitiesInterpolated = () => previewEntities;
updateShopDeliveryPreview(previewInput);
assert(previewIntent.shopDeliveryPreview.lines.length === 2, 'buy menu connects only owned selected fighters, including full carriers');
assert(previewIntent.shopDeliveryPreview.lines.every((line) => line.x === previewShop.x), 'lines use the source Shop');
previewEntities = buyers;
updateShopDeliveryPreview(previewInput);
assert(!previewIntent.shopDeliveryPreview, 'destroyed source clears buy lines');
