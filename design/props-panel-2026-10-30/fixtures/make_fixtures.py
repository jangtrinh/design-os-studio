"""Figma REST JSON fixtures for the component properties panel (design artifact, not product code).
A: reference order (Variant, State, Size, Has leading icon, Has trailing icon, Label)
B: interleaved kinds in stored order (Appearance variant, Border bool, Caption text, Density variant, Elevated bool, Footnote text, Gap variant)
C: stress (many props, long names, long values)
D: component with no properties
"""
import json, itertools
def col(r,g,b,a=1): return {"r":r/255,"g":g/255,"b":b/255,"a":a}
def text(i,name,chars,x=16,y=10,refs=None):
    d={"id":i,"name":name,"type":"TEXT","visible":True,"characters":chars,"absoluteBoundingBox":{"x":x,"y":y,"width":80,"height":20},
       "size":{"x":80,"y":20},"relativeTransform":[[1,0,x],[0,1,y]],"fills":[{"type":"SOLID","color":col(255,255,255),"opacity":1,"visible":True}],
       "style":{"fontFamily":"Inter","fontPostScriptName":"Inter-Medium","fontSize":14,"fontWeight":500,"textAlignHorizontal":"LEFT","textAlignVertical":"CENTER"}}
    if refs: d["componentPropertyReferences"]=refs
    return d
def variant_comp(cid,name,children,fill=col(37,99,235)):
    return {"id":cid,"name":name,"type":"COMPONENT","visible":True,"cornerRadius":8,
      "absoluteBoundingBox":{"x":0,"y":0,"width":112,"height":40},"size":{"x":112,"y":40},"relativeTransform":[[1,0,0],[0,1,0]],
      "fills":[{"type":"SOLID","color":fill,"opacity":1,"visible":True}],"effects":[],"children":children}
def build(fname,label,defs,values,variants,long_text=None):
    """defs: ordered list of (key,type,default,options|None)"""
    pdefs={}
    for k,t,d,o in defs:
        e={"type":t,"defaultValue":d}
        if t=="VARIANT": e["variantOptions"]=o
        pdefs[k]=e
    vnames=[n for n,_ in variants]
    comps=[]; meta={}
    for n,(vn,fill) in enumerate(variants):
        cid=f"20:{n+1}"
        comps.append(variant_comp(cid,vn,[text(f"21:{n+1}","Label",long_text or "Button",refs={"characters":"Label#1:0"} if any(k.startswith("Label") for k,_,_,_ in defs) else None)],fill))
        meta[cid]={"key":f"k{n}","name":vn,"description":"","componentSetId":"20:0"}
    cset={"id":"20:0","name":"Button","type":"COMPONENT_SET","visible":True,"absoluteBoundingBox":{"x":0,"y":240,"width":400,"height":200},
      "size":{"x":400,"y":200},"relativeTransform":[[1,0,0],[0,1,240]],"fills":[],"effects":[],"componentPropertyDefinitions":pdefs,"children":comps,
      "description":"Primary action button. Reference: ui-kit/components/Button.tsx and the full spec for the primary action across the product."}
    inst={"id":"1:2","name":"Button","type":"INSTANCE","visible":True,"componentId":"20:1","cornerRadius":8,
      "absoluteBoundingBox":{"x":60,"y":60,"width":112,"height":40},"size":{"x":112,"y":40},"relativeTransform":[[1,0,60],[0,1,60]],
      "fills":[{"type":"SOLID","color":col(37,99,235),"opacity":1,"visible":True}],"effects":[],
      "componentProperties":values,"children":[text("1:3","Label",label,refs=None)]}
    frame={"id":"1:1","name":"Props demo","type":"FRAME","visible":True,"clipsContent":True,
      "absoluteBoundingBox":{"x":0,"y":0,"width":400,"height":200},"size":{"x":400,"y":200},"relativeTransform":[[1,0,0],[0,1,0]],
      "fills":[{"type":"SOLID","color":col(245,245,247),"opacity":1,"visible":True}],"effects":[],"children":[inst]}
    doc={"name":fname,"role":"owner","lastModified":"2026-10-30T00:00:00Z","version":"1","schemaVersion":0,
      "components":meta,"componentSets":{"20:0":{"key":"ks","name":"Button","description":"Primary action button. Reference: ui-kit/components/Button.tsx and the full spec for the primary action across the product."}},"styles":{},
      "document":{"id":"0:0","name":"Document","type":"DOCUMENT","children":[{"id":"0:1","name":"Page 1","type":"CANVAS","backgroundColor":col(250,250,250),"children":[frame,cset]}]}}
    json.dump(doc,open(fname+".json","w"),indent=1)

V=[("Variant=primary, State=rest, Size=default",col(37,99,235)),("Variant=secondary, State=rest, Size=default",col(75,85,99)),("Variant=secondary, State=hover, Size=large",col(55,65,81))]
VAR=("Variant","VARIANT","secondary",["primary","secondary","ghost","destructive"])
ST=("State","VARIANT","rest",["rest","hover","pressed","disabled"])
SZ=("Size","VARIANT","default",["small","default","large"])
LI=("Has leading icon#1:1","BOOLEAN",True,None)
TI=("Has trailing icon#1:2","BOOLEAN",False,None)
LB=("Label#1:0","TEXT","Filter",None)
vals=lambda: {"Variant#1:3":{"type":"VARIANT","value":"secondary"},"State#1:4":{"type":"VARIANT","value":"rest"},"Size#1:5":{"type":"VARIANT","value":"default"},
  "Has leading icon#1:1":{"type":"BOOLEAN","value":True},"Has trailing icon#1:2":{"type":"BOOLEAN","value":False},"Label#1:0":{"type":"TEXT","value":"Filter"}}
build("props-A-reference-order","Filter",[VAR,ST,SZ,LI,TI,LB],vals(),V)
# B: names chosen so the stored (alphabetical) definition order interleaves kinds:
# Disabled(bool), Has icon(bool)... -> see below. Imported Figma maps are alphabetical by key (serde_json without preserve_order).
BD=[("Appearance","VARIANT","secondary",["primary","secondary","ghost"]),
    ("Border#4:0","BOOLEAN",True,None),
    ("Caption#4:1","TEXT","Filter",None),
    ("Density","VARIANT","default",["compact","default","comfy"]),
    ("Elevated#4:2","BOOLEAN",False,None),
    ("Footnote#4:3","TEXT","Hint",None),
    ("Gap","VARIANT","md",["sm","md","lg"])]
bv={"Appearance":{"type":"VARIANT","value":"secondary"},"Border#4:0":{"type":"BOOLEAN","value":True},"Caption#4:1":{"type":"TEXT","value":"Filter"},
    "Density":{"type":"VARIANT","value":"default"},"Elevated#4:2":{"type":"BOOLEAN","value":False},"Footnote#4:3":{"type":"TEXT","value":"Hint"},"Gap":{"type":"VARIANT","value":"md"}}
build("props-B-mixed-order","Filter",BD,bv,V)
# C stress
defs=[("Variant","VARIANT","secondary",["primary","secondary","ghost","destructive"]),
 ("A very long variant property name that cannot possibly fit the label column","VARIANT","a-very-long-option-value-that-overflows-the-control",["a-very-long-option-value-that-overflows-the-control","short"]),
 ("State","VARIANT","rest",["rest","hover","pressed","disabled"]),("Size","VARIANT","default",["small","default","large"])]
vv={"Variant":{"type":"VARIANT","value":"secondary"},"A very long variant property name that cannot possibly fit the label column":{"type":"VARIANT","value":"a-very-long-option-value-that-overflows-the-control"},"State":{"type":"VARIANT","value":"rest"},"Size":{"type":"VARIANT","value":"default"}}
for i in range(1,7):
    k=f"Has icon {i}" if i!=2 else "Has a trailing icon with a really long name"
    defs.append((k+f"#2:{i}","BOOLEAN",i%2==0,None)); vv[k+f"#2:{i}"]={"type":"BOOLEAN","value":i%2==0}
for i,nm in enumerate(["Label","Helper text that is rather long to read","Placeholder"]):
    defs.append((nm+f"#3:{i}","TEXT","Filter" if i==0 else "A very long default text value that goes on well past the end of the field and must truncate",None))
    vv[nm+f"#3:{i}"]={"type":"TEXT","value":"Filter" if i==0 else "A very long default text value that goes on well past the end of the field and must truncate"}
build("props-C-stress","Filter",[(k,t,d,o) for k,t,d,o in defs],vv,V)
# D: no props
doc=json.load(open("props-A-reference-order.json"))
for n in doc["document"]["children"][0]["children"]:
    if n["type"]=="COMPONENT_SET": n["componentPropertyDefinitions"]={}
    if n["id"]=="1:1": n["children"][0]["componentProperties"]={}
json.dump(doc,open("props-D-no-props.json","w"),indent=1)
# NOTE: the REST importer reads a description from the node itself ("description"/"symbolDescription"), not from the top-level components/componentSets metadata maps, so the fixtures put it on the COMPONENT_SET node too.
# E: like A, but neither the component nor its set has a description (spec D5/AC16: description row omitted)
doc=json.load(open("props-A-reference-order.json"))
for k in doc["componentSets"]: doc["componentSets"][k]["description"]=""
for n in doc["document"]["children"][0]["children"]:
    if n["type"]=="COMPONENT_SET": n["description"]=""
for k in doc["components"]: doc["components"][k]["description"]=""
json.dump(doc,open("props-E-empty-description.json","w"),indent=1)
